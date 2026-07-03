//! Optimization boundary.
//!
//! The optimizer may use known heuristics internally, but it should return Freecut-owned solution
//! data rather than leaking an external library's result shape into the rest of the application.

use crate::{
    domain::{LayoutKind, PatternDirection, PieceId, Project},
    render::{
        Cut as GuideCut, CutOrientation, LeafKind as GuideLeafKind, PlacedPiece, Rect,
        SliceNode as GuideSliceNode, Solution, SolutionSheet,
    },
};

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub trait Optimizer {
    #[allow(clippy::missing_errors_doc)]
    fn optimize(&self, project: &Project) -> Result<Solution, OptimizeError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizeError {
    EmptyInput,
    NoSolution,
    InvalidProject(String),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BaselineOptimizer;

impl BaselineOptimizer {
    #[allow(clippy::missing_errors_doc)]
    pub fn optimize_with_config(
        &self,
        project: &Project,
        config: OptimizerConfig,
    ) -> Result<Solution, OptimizeError> {
        let instance = expand_project(project)?;
        if instance.cuts.is_empty() {
            return Ok(Solution {
                layout: project.settings.layout,
                sheets: Vec::new(),
                fitness: Some(1.0),
            });
        }

        match project.settings.layout {
            LayoutKind::Guillotine => optimize_population_pipeline::<BaselineGuillotineBackend>(
                &instance,
                &guillotine_pipeline_config(config),
                LayoutKind::Guillotine,
            ),
            LayoutKind::Nested => optimize_population_pipeline::<NestedMaxRectsBackend>(
                &instance,
                &PopulationPipelineConfig::from_optimizer_config(config),
                LayoutKind::Nested,
            ),
        }
    }
}

fn guillotine_pipeline_config(config: OptimizerConfig) -> PopulationPipelineConfig {
    let mut pipeline_config = PopulationPipelineConfig::from_optimizer_config(config);

    if config.effort == OptimizerEffort::Balanced {
        pipeline_config.initial.include_heuristic_variants = true;
        pipeline_config.initial.max_candidates = Some(
            1 + BaselineGuillotineBackend::heuristic_variants()
                .len()
                .saturating_sub(1)
                + pipeline_config.initial.shuffled_candidate_count,
        );
    }

    pipeline_config
}

impl Optimizer for BaselineOptimizer {
    fn optimize(&self, project: &Project) -> Result<Solution, OptimizeError> {
        self.optimize_with_config(project, OptimizerConfig::default())
    }
}

trait PackingBackend {
    type Bin;
    type Heuristic: Copy;

    fn default_heuristic() -> Self::Heuristic;
    fn heuristic_variants() -> Vec<Self::Heuristic> {
        vec![Self::default_heuristic()]
    }
    fn new_bin(stock: StockInstance, kerf_width: u32) -> Self::Bin;
    fn insert(bin: &mut Self::Bin, cut: &CutInstance, heuristic: Self::Heuristic) -> bool;
    fn clone_stock_instance(bin: &Self::Bin) -> StockInstance;
    fn stock_key(bin: &Self::Bin) -> StockInstanceKey {
        StockInstanceKey::from(&Self::clone_stock_instance(bin))
    }
    fn placed_cut_keys(bin: &Self::Bin) -> Vec<CutInstanceKey>;
    fn stock_area(bin: &Self::Bin) -> u64;
    fn fitness(bin: &Self::Bin) -> f64;
    fn into_solution_sheet(bin: Self::Bin) -> SolutionSheet;
}

fn optimize_population_pipeline<B>(
    instance: &ProblemInstance,
    config: &PopulationPipelineConfig,
    layout: LayoutKind,
) -> Result<Solution, OptimizeError>
where
    B: PackingBackend,
{
    basic_feasibility_prefilter(instance)?;

    let kerf_width = instance.kerf_width;
    let population = initial_population::<B>(instance, &config.initial);
    let population =
        run_population_generations(population, config, kerf_width, B::default_heuristic());

    select_best_valid_candidate(population)
        .map(|candidate| candidate.into_solution(layout))
        .ok_or(OptimizeError::NoSolution)
}

fn basic_feasibility_prefilter(instance: &ProblemInstance) -> Result<(), OptimizeError> {
    if !instance.cuts.iter().all(|cut| {
        instance
            .stock
            .iter()
            .any(|stock| cut_fits_stock(cut, stock))
    }) {
        return Err(OptimizeError::NoSolution);
    }

    if total_cut_area(&instance.cuts) > total_stock_area(&instance.stock) {
        return Err(OptimizeError::NoSolution);
    }

    Ok(())
}

fn total_cut_area(cuts: &[CutInstance]) -> u64 {
    cuts.iter()
        .map(|cut| u64::from(cut.width) * u64::from(cut.length))
        .sum()
}

fn total_stock_area(stock: &[StockInstance]) -> u64 {
    stock
        .iter()
        .map(|stock| u64::from(stock.width) * u64::from(stock.length))
        .sum()
}

fn cut_fits_stock(cut: &CutInstance, stock: &StockInstance) -> bool {
    fit_cut_in_rect(
        cut,
        stock.pattern,
        Rect {
            x: 0,
            y: 0,
            width: stock.width,
            length: stock.length,
        },
    )
    .is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StockInventory {
    stock: Vec<StockInstance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StockInstanceKey {
    stock_id: PieceId,
    instance: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CutInstanceKey {
    cut_id: PieceId,
    instance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlacedCutRecord {
    key: CutInstanceKey,
    cut: CutInstance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InitialPopulationConfig {
    seed: u64,
    include_sorted_first_fit: bool,
    include_shuffled_first_fit: bool,
    shuffled_candidate_count: usize,
    include_heuristic_variants: bool,
    max_candidates: Option<usize>,
}

impl Default for InitialPopulationConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: false,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PopulationConfig {
    epochs: u32,
    survivor_limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepairConfig {
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CrossoverConfig {
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompactionConfig {
    enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PopulationPipelineConfig {
    initial: InitialPopulationConfig,
    crossover: CrossoverConfig,
    repair: RepairConfig,
    compaction: CompactionConfig,
    population: PopulationConfig,
}

impl PopulationPipelineConfig {
    fn from_optimizer_config(config: OptimizerConfig) -> Self {
        match config.effort {
            OptimizerEffort::Fast => Self::default(),
            OptimizerEffort::Balanced => Self::genetic_default(0),
            OptimizerEffort::Thorough => Self::thorough_default(0),
        }
    }

    fn genetic_default(seed: u64) -> Self {
        Self {
            initial: InitialPopulationConfig {
                seed,
                include_sorted_first_fit: true,
                include_shuffled_first_fit: true,
                shuffled_candidate_count: 8,
                include_heuristic_variants: false,
                max_candidates: Some(9),
            },
            crossover: CrossoverConfig { enabled: true },
            repair: RepairConfig { enabled: true },
            compaction: CompactionConfig { enabled: true },
            population: PopulationConfig {
                epochs: 1,
                survivor_limit: Some(16),
            },
        }
    }

    fn thorough_default(seed: u64) -> Self {
        Self {
            initial: InitialPopulationConfig {
                seed,
                include_sorted_first_fit: true,
                include_shuffled_first_fit: true,
                shuffled_candidate_count: 24,
                include_heuristic_variants: true,
                max_candidates: Some(29),
            },
            crossover: CrossoverConfig { enabled: true },
            repair: RepairConfig { enabled: true },
            compaction: CompactionConfig { enabled: true },
            population: PopulationConfig {
                epochs: 4,
                survivor_limit: Some(32),
            },
        }
    }
}

impl Default for PopulationPipelineConfig {
    fn default() -> Self {
        Self {
            initial: InitialPopulationConfig::default(),
            crossover: CrossoverConfig { enabled: false },
            repair: RepairConfig { enabled: false },
            compaction: CompactionConfig { enabled: false },
            population: PopulationConfig {
                epochs: 0,
                survivor_limit: None,
            },
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum CandidateSeedDescription {
    SortedFirstFit,
    ShuffledFirstFit { seed: u64, index: usize },
    HeuristicVariant { index: usize },
}

#[derive(Debug, Clone)]
struct CandidateSeed<H> {
    cut_order: Vec<CutInstanceKey>,
    heuristic: H,
    #[cfg(test)]
    description: CandidateSeedDescription,
}

#[derive(Debug, Clone, Copy)]
struct TinyPrng {
    state: u64,
}

impl TinyPrng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn next_index(&mut self, upper_exclusive: usize) -> usize {
        debug_assert!(upper_exclusive > 0);
        let upper_exclusive = u64::try_from(upper_exclusive).expect("usize fits into u64");
        let index = self.next_u64() % upper_exclusive;
        usize::try_from(index).expect("random index is below upper_exclusive")
    }
}

// Fitness is only a ranking/display scalar; integer geometry remains the source of truth.
#[allow(clippy::cast_precision_loss)]
fn area_as_fitness_weight(area: u64) -> f64 {
    area as f64
}

impl From<&StockInstance> for StockInstanceKey {
    fn from(stock: &StockInstance) -> Self {
        Self {
            stock_id: stock.stock_id,
            instance: stock.instance,
        }
    }
}

impl From<&CutInstance> for CutInstanceKey {
    fn from(cut: &CutInstance) -> Self {
        Self {
            cut_id: cut.cut_id,
            instance: cut.instance,
        }
    }
}

impl From<&PlacedPiece> for CutInstanceKey {
    fn from(piece: &PlacedPiece) -> Self {
        Self {
            cut_id: piece.cut_id,
            instance: piece.instance,
        }
    }
}

fn sorted_first_fit_candidate_seed<B>(cuts: &[CutInstance]) -> CandidateSeed<B::Heuristic>
where
    B: PackingBackend,
{
    CandidateSeed {
        cut_order: sorted_cut_order(cuts),
        heuristic: B::default_heuristic(),
        #[cfg(test)]
        description: CandidateSeedDescription::SortedFirstFit,
    }
}

fn sorted_cut_order(cuts: &[CutInstance]) -> Vec<CutInstanceKey> {
    let mut cuts = cuts.to_vec();
    cuts.sort_by(compare_cut_instances_for_first_fit);
    cuts.iter().map(CutInstanceKey::from).collect()
}

fn shuffle_cut_order(cuts: &[CutInstance], seed: u64) -> Vec<CutInstanceKey> {
    let mut cut_order = sorted_cut_order(cuts);
    let mut prng = TinyPrng::new(seed);

    for index in (1..cut_order.len()).rev() {
        let swap_index = prng.next_index(index + 1);
        cut_order.swap(index, swap_index);
    }

    cut_order
}

fn candidate_from_seed<B>(
    instance: &ProblemInstance,
    seed: &CandidateSeed<B::Heuristic>,
) -> Option<Candidate<B>>
where
    B: PackingBackend,
{
    let mut candidate = Candidate::new(
        StockInventory::new(instance.stock.clone()),
        CutCatalog::new(instance.cuts.clone()),
    );
    let ordered_cuts = candidate.cuts_for_keys(&seed.cut_order)?;

    for cut in &ordered_cuts {
        candidate.place_cut_first_fit(cut, instance.kerf_width, seed.heuristic);
    }

    Some(candidate)
}

fn initial_candidate_seeds<B>(
    cuts: &[CutInstance],
    config: &InitialPopulationConfig,
) -> Vec<CandidateSeed<B::Heuristic>>
where
    B: PackingBackend,
{
    let mut seeds = Vec::new();

    if config.include_sorted_first_fit {
        seeds.push(sorted_first_fit_candidate_seed::<B>(cuts));
    }

    if config.include_heuristic_variants {
        for (_index, heuristic) in B::heuristic_variants().into_iter().enumerate().skip(1) {
            seeds.push(CandidateSeed {
                cut_order: sorted_cut_order(cuts),
                heuristic,
                #[cfg(test)]
                description: CandidateSeedDescription::HeuristicVariant { index: _index },
            });
        }
    }

    if config.include_shuffled_first_fit {
        for index in 0..config.shuffled_candidate_count {
            let seed = config.seed.wrapping_add(index as u64);
            seeds.push(CandidateSeed {
                cut_order: shuffle_cut_order(cuts, seed),
                heuristic: B::default_heuristic(),
                #[cfg(test)]
                description: CandidateSeedDescription::ShuffledFirstFit { seed, index },
            });
        }
    }

    if let Some(max_candidates) = config.max_candidates {
        seeds.truncate(max_candidates);
    }

    seeds
}

fn initial_population<B>(
    instance: &ProblemInstance,
    config: &InitialPopulationConfig,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    initial_candidate_seeds::<B>(&instance.cuts, config)
        .iter()
        .filter_map(|seed| candidate_from_seed::<B>(instance, seed))
        .collect()
}

fn crossover_population<B>(
    population: Vec<Candidate<B>>,
    config: CrossoverConfig,
    kerf_width: u32,
    heuristic: B::Heuristic,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    if !config.enabled {
        return population;
    }

    let children = population
        .chunks(2)
        .filter_map(|parents| match parents {
            [left, right] => {
                let donor_bin_index = best_donor_bin_index(right)?;
                crossover_child_from_donor_bin(left, right, donor_bin_index, kerf_width, heuristic)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut population = population;
    population.extend(children);
    population
}

fn best_donor_bin_index<B>(donor: &Candidate<B>) -> Option<usize>
where
    B: PackingBackend,
{
    donor
        .bins
        .iter()
        .enumerate()
        .reduce(|best, candidate| {
            if B::fitness(candidate.1)
                .partial_cmp(&B::fitness(best.1))
                .unwrap_or(Ordering::Equal)
                == Ordering::Greater
            {
                candidate
            } else {
                best
            }
        })
        .map(|(index, _)| index)
}

fn crossover_child_from_donor_bin<B>(
    stock_source: &Candidate<B>,
    donor: &Candidate<B>,
    donor_bin_index: usize,
    kerf_width: u32,
    heuristic: B::Heuristic,
) -> Option<Candidate<B>>
where
    B: PackingBackend,
{
    let donor_bin = donor.bins.get(donor_bin_index)?;
    let donor_stock = B::clone_stock_instance(donor_bin);
    let donor_stock_key = StockInstanceKey::from(&donor_stock);
    let donor_cut_keys = B::placed_cut_keys(donor_bin);
    let donor_cuts = stock_source.cuts_for_keys(&donor_cut_keys)?;

    let mut rebuilt_donor_bin = B::new_bin(donor_stock, kerf_width);
    for cut in &donor_cuts {
        if !B::insert(&mut rebuilt_donor_bin, cut, heuristic) {
            return None;
        }
    }

    let mut available_stock = stock_source.full_stock_instances();
    let donor_stock_index = available_stock
        .iter()
        .position(|stock| StockInstanceKey::from(stock) == donor_stock_key)?;
    available_stock.remove(donor_stock_index);

    let mut child = Candidate::new(
        StockInventory::new(available_stock),
        stock_source.cut_catalog.clone(),
    );
    child.bins.push(rebuilt_donor_bin);
    child.unused_cuts = stock_source
        .cut_catalog
        .cuts
        .iter()
        .filter(|cut| !donor_cut_keys.contains(&CutInstanceKey::from(*cut)))
        .cloned()
        .collect();

    Some(child)
}

fn repair_population<B>(
    population: Vec<Candidate<B>>,
    config: RepairConfig,
    kerf_width: u32,
    heuristic: B::Heuristic,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    if !config.enabled {
        return population;
    }

    population
        .into_iter()
        .map(|mut candidate| {
            candidate.reinsert_unused_first_fit(kerf_width, heuristic);
            candidate
        })
        .collect()
}

fn compact_population<B>(
    population: Vec<Candidate<B>>,
    config: CompactionConfig,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    if !config.enabled {
        return population;
    }

    population
        .into_iter()
        .map(|mut candidate| {
            candidate.compact_worst_bin_for_repair();
            candidate
        })
        .collect()
}

fn run_population_generations<B>(
    population: Vec<Candidate<B>>,
    config: &PopulationPipelineConfig,
    kerf_width: u32,
    heuristic: B::Heuristic,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    let mut population = population;

    for _ in 0..config.population.epochs {
        population = crossover_population(population, config.crossover, kerf_width, heuristic);
        population = repair_population(population, config.repair, kerf_width, heuristic);
        population = compact_population(population, config.compaction);
        population = repair_population(population, config.repair, kerf_width, heuristic);
        population = survival_generation(population, config.population.survivor_limit);
    }

    population
}

#[cfg(test)]
fn run_population_epochs<B>(
    population: Vec<Candidate<B>>,
    config: &PopulationConfig,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    let mut population = population;

    for _ in 0..config.epochs {
        population = survival_generation(population, config.survivor_limit);
    }

    population
}

fn survival_generation<B>(
    population: Vec<Candidate<B>>,
    survivor_limit: Option<usize>,
) -> Vec<Candidate<B>>
where
    B: PackingBackend,
{
    let mut survivors = population
        .into_iter()
        .filter(Candidate::is_valid)
        .collect::<Vec<_>>();

    survivors.sort_by(|left, right| compare_optional_candidate_score(right.score(), left.score()));

    if let Some(limit) = survivor_limit {
        survivors.truncate(limit);
    }

    survivors
}

fn select_best_valid_candidate<B>(population: Vec<Candidate<B>>) -> Option<Candidate<B>>
where
    B: PackingBackend,
{
    population
        .into_iter()
        .filter(Candidate::is_valid)
        .reduce(|best, candidate| {
            if compare_optional_candidate_score(candidate.score(), best.score())
                == Ordering::Greater
            {
                candidate
            } else {
                best
            }
        })
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CandidateScore {
    used_stock_count: usize,
    waste_area: u64,
    fitness: Option<f64>,
}

fn compare_optional_candidate_score(
    left: Option<CandidateScore>,
    right: Option<CandidateScore>,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_candidate_score(left, right),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn compare_candidate_score(left: CandidateScore, right: CandidateScore) -> Ordering {
    right
        .used_stock_count
        .cmp(&left.used_stock_count)
        .then_with(|| right.waste_area.cmp(&left.waste_area))
        .then_with(|| compare_optional_fitness(left.fitness, right.fitness))
}

fn compare_optional_fitness(left: Option<f64>, right: Option<f64>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CutCatalog {
    cuts: Vec<CutInstance>,
}

impl CutCatalog {
    fn new(cuts: Vec<CutInstance>) -> Self {
        Self { cuts }
    }

    fn cut(&self, key: CutInstanceKey) -> Option<&CutInstance> {
        self.cuts
            .iter()
            .find(|cut| CutInstanceKey::from(*cut) == key)
    }
}

impl StockInventory {
    fn new(stock: Vec<StockInstance>) -> Self {
        Self { stock }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.stock.len()
    }

    fn return_stock(&mut self, stock: StockInstance) {
        self.stock.push(stock);
    }

    fn take_first_fitting_stock<B>(
        &mut self,
        cut: &CutInstance,
        kerf_width: u32,
        heuristic: B::Heuristic,
    ) -> Option<B::Bin>
    where
        B: PackingBackend,
    {
        for stock_index in 0..self.stock.len() {
            let stock = self.stock[stock_index].clone();
            let mut bin = B::new_bin(stock, kerf_width);
            if B::insert(&mut bin, cut, heuristic) {
                self.stock.remove(stock_index);
                return Some(bin);
            }
        }

        None
    }
}

struct Candidate<B>
where
    B: PackingBackend,
{
    bins: Vec<B::Bin>,
    available_stock: StockInventory,
    unused_cuts: Vec<CutInstance>,
    cut_catalog: CutCatalog,
}

impl<B> Candidate<B>
where
    B: PackingBackend,
{
    fn new(available_stock: StockInventory, cut_catalog: CutCatalog) -> Self {
        Self {
            bins: Vec::new(),
            available_stock,
            unused_cuts: Vec::new(),
            cut_catalog,
        }
    }

    fn place_cut_first_fit(
        &mut self,
        cut: &CutInstance,
        kerf_width: u32,
        heuristic: B::Heuristic,
    ) -> bool {
        for bin in &mut self.bins {
            if B::insert(bin, cut, heuristic) {
                self.remove_unused_cut(CutInstanceKey::from(cut));
                return true;
            }
        }

        if let Some(bin) = self
            .available_stock
            .take_first_fitting_stock::<B>(cut, kerf_width, heuristic)
        {
            self.bins.push(bin);
            self.remove_unused_cut(CutInstanceKey::from(cut));
            return true;
        }

        self.unused_cuts.push(cut.clone());
        false
    }

    #[cfg(test)]
    fn remove_placed_cuts_from_bin(
        &mut self,
        bin_index: usize,
        keys: &[CutInstanceKey],
        kerf_width: u32,
        heuristic: B::Heuristic,
    ) -> Option<Vec<PlacedCutRecord>> {
        let bin = self.bins.get(bin_index)?;
        let stock = B::clone_stock_instance(bin);
        let placed_keys = B::placed_cut_keys(bin);
        let removed_keys = placed_keys
            .iter()
            .copied()
            .filter(|key| keys.contains(key))
            .collect::<Vec<_>>();

        if removed_keys.is_empty() {
            return Some(Vec::new());
        }

        let remaining_keys = placed_keys
            .into_iter()
            .filter(|key| !keys.contains(key))
            .collect::<Vec<_>>();
        let remaining_cuts = self.cuts_for_keys(&remaining_keys)?;
        let removed_cuts = self.cuts_for_keys(&removed_keys)?;

        let mut rebuilt_bin = B::new_bin(stock, kerf_width);
        for cut in &remaining_cuts {
            if !B::insert(&mut rebuilt_bin, cut, heuristic) {
                return None;
            }
        }

        self.bins[bin_index] = rebuilt_bin;
        self.unused_cuts.extend(removed_cuts.iter().cloned());

        Some(
            removed_keys
                .into_iter()
                .zip(removed_cuts)
                .map(|(key, cut)| PlacedCutRecord { key, cut })
                .collect(),
        )
    }

    fn remove_unused_cut(&mut self, key: CutInstanceKey) {
        self.unused_cuts
            .retain(|cut| CutInstanceKey::from(cut) != key);
    }

    fn reinsert_unused_first_fit(&mut self, kerf_width: u32, heuristic: B::Heuristic) -> usize {
        let unused_cuts = std::mem::take(&mut self.unused_cuts);
        let mut reinserted = 0;

        for cut in unused_cuts {
            if self.place_cut_first_fit(&cut, kerf_width, heuristic) {
                reinserted += 1;
            }
        }

        reinserted
    }

    fn compact_worst_bin_for_repair(&mut self) -> Option<usize> {
        if self.bins.len() < 2 {
            return None;
        }

        let bin_index = self.worst_bin_index()?;
        let bin = self.bins.get(bin_index)?;
        let stock = B::clone_stock_instance(bin);
        let cut_keys = B::placed_cut_keys(bin);
        let removed_cuts = self.cuts_for_keys(&cut_keys)?;

        self.bins.remove(bin_index);
        self.available_stock.return_stock(stock);
        self.unused_cuts.extend(removed_cuts);

        Some(cut_keys.len())
    }

    fn worst_bin_index(&self) -> Option<usize> {
        self.bins
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                B::fitness(left)
                    .partial_cmp(&B::fitness(right))
                    .unwrap_or(Ordering::Equal)
            })
            .map(|(index, _)| index)
    }

    fn is_valid(&self) -> bool {
        self.unused_cuts.is_empty()
            && !self.has_duplicate_placed_cut_keys()
            && !self.has_unresolved_placed_cut_keys()
    }

    fn has_duplicate_placed_cut_keys(&self) -> bool {
        let placed_cut_keys = self.placed_cut_keys();
        placed_cut_keys.iter().enumerate().any(|(index, key)| {
            placed_cut_keys
                .iter()
                .skip(index + 1)
                .any(|other| other == key)
        })
    }

    fn has_unresolved_placed_cut_keys(&self) -> bool {
        self.placed_cut_records().is_none()
    }

    fn cut_for_key(&self, key: CutInstanceKey) -> Option<&CutInstance> {
        self.cut_catalog.cut(key)
    }

    fn cuts_for_keys(&self, keys: &[CutInstanceKey]) -> Option<Vec<CutInstance>> {
        keys.iter()
            .map(|key| self.cut_for_key(*key).cloned())
            .collect()
    }

    fn placed_cut_records(&self) -> Option<Vec<PlacedCutRecord>> {
        let keys = self.placed_cut_keys();
        let cuts = self.cuts_for_keys(&keys)?;

        Some(
            keys.into_iter()
                .zip(cuts)
                .map(|(key, cut)| PlacedCutRecord { key, cut })
                .collect(),
        )
    }

    #[cfg(test)]
    fn remaining_stock_count(&self) -> usize {
        self.available_stock.len()
    }

    fn used_stock_keys(&self) -> Vec<StockInstanceKey> {
        self.bins.iter().map(B::stock_key).collect()
    }

    fn used_stock_instances(&self) -> Vec<StockInstance> {
        self.bins.iter().map(B::clone_stock_instance).collect()
    }

    fn full_stock_instances(&self) -> Vec<StockInstance> {
        let mut stock = self.used_stock_instances();
        stock.extend(self.available_stock.stock.clone());
        stock
    }

    fn placed_cut_keys(&self) -> Vec<CutInstanceKey> {
        self.bins.iter().flat_map(B::placed_cut_keys).collect()
    }

    fn placed_cut_area(&self) -> Option<u64> {
        Some(
            self.placed_cut_records()?
                .iter()
                .map(|record| u64::from(record.cut.width) * u64::from(record.cut.length))
                .sum(),
        )
    }

    fn used_stock_area(&self) -> u64 {
        self.bins.iter().map(B::stock_area).sum()
    }

    fn score(&self) -> Option<CandidateScore> {
        let used_stock_area = self.used_stock_area();
        let placed_cut_area = self.placed_cut_area()?;

        Some(CandidateScore {
            used_stock_count: self.bins.len(),
            waste_area: used_stock_area.saturating_sub(placed_cut_area),
            fitness: self.fitness(),
        })
    }

    fn fitness(&self) -> Option<f64> {
        let total_sheet_area = self.used_stock_area();
        let weighted_fitness = self
            .bins
            .iter()
            .map(|bin| B::fitness(bin) * area_as_fitness_weight(B::stock_area(bin)))
            .sum::<f64>();

        if total_sheet_area == 0 {
            None
        } else {
            Some(weighted_fitness / area_as_fitness_weight(total_sheet_area))
        }
    }

    fn into_solution(self, layout: LayoutKind) -> Solution {
        debug_assert_eq!(self.used_stock_keys().len(), self.bins.len());
        debug_assert_eq!(self.used_stock_instances().len(), self.bins.len());

        let fitness = self.fitness();
        Solution {
            layout,
            sheets: self.bins.into_iter().map(B::into_solution_sheet).collect(),
            fitness,
        }
    }
}

fn compare_cut_instances_for_first_fit(left: &CutInstance, right: &CutInstance) -> Ordering {
    right
        .width
        .cmp(&left.width)
        .then_with(|| right.length.cmp(&left.length))
        .then_with(|| left.cut_id.0.cmp(&right.cut_id.0))
        .then_with(|| left.instance.cmp(&right.instance))
}

fn compare_stock_instances_for_first_fit(left: &StockInstance, right: &StockInstance) -> Ordering {
    right
        .width
        .cmp(&left.width)
        .then_with(|| right.length.cmp(&left.length))
        .then_with(|| left.stock_id.0.cmp(&right.stock_id.0))
        .then_with(|| left.instance.cmp(&right.instance))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BaselineGuillotineHeuristic {
    rect_choice: GuillotineRectChoice,
    split: GuillotineSplitHeuristic,
    rotation: RotationPreference,
}

impl BaselineGuillotineHeuristic {
    const fn new(
        rect_choice: GuillotineRectChoice,
        split: GuillotineSplitHeuristic,
        rotation: RotationPreference,
    ) -> Self {
        Self {
            rect_choice,
            split,
            rotation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuillotineRectChoice {
    Area,
    ShortSide,
    LongSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuillotineSplitHeuristic {
    Horizontal,
    LongerAxis,
    MaximizeArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RotationPreference {
    PreferUpright,
    PreferRotated,
}

struct BaselineGuillotineBackend;

impl PackingBackend for BaselineGuillotineBackend {
    type Bin = GuillotineSheet;
    type Heuristic = BaselineGuillotineHeuristic;

    fn default_heuristic() -> Self::Heuristic {
        BaselineGuillotineHeuristic::new(
            GuillotineRectChoice::Area,
            GuillotineSplitHeuristic::Horizontal,
            RotationPreference::PreferUpright,
        )
    }

    fn heuristic_variants() -> Vec<Self::Heuristic> {
        vec![
            Self::default_heuristic(),
            BaselineGuillotineHeuristic::new(
                GuillotineRectChoice::LongSide,
                GuillotineSplitHeuristic::LongerAxis,
                RotationPreference::PreferUpright,
            ),
            BaselineGuillotineHeuristic::new(
                GuillotineRectChoice::LongSide,
                GuillotineSplitHeuristic::MaximizeArea,
                RotationPreference::PreferUpright,
            ),
            BaselineGuillotineHeuristic::new(
                GuillotineRectChoice::ShortSide,
                GuillotineSplitHeuristic::LongerAxis,
                RotationPreference::PreferUpright,
            ),
            BaselineGuillotineHeuristic::new(
                GuillotineRectChoice::LongSide,
                GuillotineSplitHeuristic::LongerAxis,
                RotationPreference::PreferRotated,
            ),
        ]
    }

    fn new_bin(stock: StockInstance, kerf_width: u32) -> Self::Bin {
        GuillotineSheet::new(stock, kerf_width)
    }

    fn insert(bin: &mut Self::Bin, cut: &CutInstance, heuristic: Self::Heuristic) -> bool {
        bin.try_insert(cut, heuristic)
    }

    fn clone_stock_instance(bin: &Self::Bin) -> StockInstance {
        bin.stock.clone()
    }

    fn placed_cut_keys(bin: &Self::Bin) -> Vec<CutInstanceKey> {
        bin.placed_pieces.iter().map(CutInstanceKey::from).collect()
    }

    fn stock_area(bin: &Self::Bin) -> u64 {
        u64::from(bin.stock.width) * u64::from(bin.stock.length)
    }

    fn fitness(bin: &Self::Bin) -> f64 {
        let stock_area = Self::stock_area(bin);
        if stock_area == 0 {
            return 0.0;
        }

        let placed_area = bin
            .placed_pieces
            .iter()
            .map(|piece| u64::from(piece.rect.width) * u64::from(piece.rect.length))
            .sum::<u64>();

        area_as_fitness_weight(placed_area) / area_as_fitness_weight(stock_area)
    }

    fn into_solution_sheet(bin: Self::Bin) -> SolutionSheet {
        bin.into_solution_sheet()
    }
}

#[derive(Debug, Clone)]
struct GuillotineSheet {
    stock: StockInstance,
    kerf_width: u32,
    free_rects: Vec<Rect>,
    placed_pieces: Vec<PlacedPiece>,
    // Transitional Phase-3 state: kept only while every chosen free rect still
    // corresponds to a real free leaf. If legacy FreeRect merging creates a
    // synthetic search rect, this is set to None instead of carrying a false guide.
    slicing_tree: Option<GuillotineSliceNode>,
}

impl GuillotineSheet {
    fn new(stock: StockInstance, kerf_width: u32) -> Self {
        let stock_rect = Rect {
            x: 0,
            y: 0,
            width: stock.width,
            length: stock.length,
        };

        Self {
            free_rects: vec![stock_rect],
            stock,
            kerf_width,
            placed_pieces: Vec::new(),
            slicing_tree: Some(GuillotineSliceNode::free(stock_rect)),
        }
    }

    fn try_insert(&mut self, cut: &CutInstance, heuristic: BaselineGuillotineHeuristic) -> bool {
        let Some((free_index, free_rect, fit)) = self
            .free_rects
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(index, free_rect)| {
                fit_cut_in_rects(cut, self.stock.pattern, free_rect, heuristic.rotation)
                    .into_iter()
                    .map(move |fit| (index, free_rect, fit))
            })
            .min_by_key(|(_, free_rect, fit)| {
                free_rect_choice_score(*free_rect, fit.rect, heuristic.rect_choice)
            })
        else {
            return false;
        };

        let split_direction = split_direction(free_rect, fit.rect, heuristic.split);
        let subtree =
            guillotine_insert_subtree(free_rect, fit.rect, cut, split_direction, self.kerf_width);

        self.free_rects.swap_remove(free_index);
        self.split_free_rect_in_direction(free_rect, fit.rect, split_direction);
        if let Some(tree) = &mut self.slicing_tree {
            if let Some(subtree) = subtree {
                if !tree.replace_free_leaf(free_rect, subtree) {
                    self.slicing_tree = None;
                }
            } else {
                self.slicing_tree = None;
            }
        }
        self.merge_free_rects_and_invalidate_slicing_tree();
        self.placed_pieces.push(PlacedPiece {
            cut_id: cut.cut_id,
            instance: cut.instance,
            rect: fit.rect,
            pattern: fit.pattern,
            rotated: fit.rotated,
        });
        true
    }

    #[cfg(test)]
    fn split_free_rect(&mut self, free_rect: Rect, placed: Rect, split: GuillotineSplitHeuristic) {
        self.split_free_rect_in_direction(
            free_rect,
            placed,
            split_direction(free_rect, placed, split),
        );
    }

    fn split_free_rect_in_direction(
        &mut self,
        free_rect: Rect,
        placed: Rect,
        direction: GuillotineSplitDirection,
    ) {
        match direction {
            GuillotineSplitDirection::Horizontal => self.split_horizontal(free_rect, placed),
            GuillotineSplitDirection::Vertical => self.split_vertical(free_rect, placed),
        }
    }

    fn split_horizontal(&mut self, free_rect: Rect, placed: Rect) {
        let remaining_length = free_rect.length - placed.length;
        if remaining_length > self.kerf_width {
            self.free_rects.push(Rect {
                x: free_rect.x,
                y: free_rect.y + placed.length + self.kerf_width,
                width: free_rect.width,
                length: remaining_length - self.kerf_width,
            });
        }

        let remaining_width = free_rect.width - placed.width;
        if remaining_width > self.kerf_width {
            self.free_rects.push(Rect {
                x: free_rect.x + placed.width + self.kerf_width,
                y: free_rect.y,
                width: remaining_width - self.kerf_width,
                length: placed.length,
            });
        }
    }

    fn split_vertical(&mut self, free_rect: Rect, placed: Rect) {
        let remaining_length = free_rect.length - placed.length;
        if remaining_length > self.kerf_width {
            self.free_rects.push(Rect {
                x: free_rect.x,
                y: free_rect.y + placed.length + self.kerf_width,
                width: placed.width,
                length: remaining_length - self.kerf_width,
            });
        }

        let remaining_width = free_rect.width - placed.width;
        if remaining_width > self.kerf_width {
            self.free_rects.push(Rect {
                x: free_rect.x + placed.width + self.kerf_width,
                y: free_rect.y,
                width: remaining_width - self.kerf_width,
                length: free_rect.length,
            });
        }
    }

    fn merge_free_rects(&mut self) -> bool {
        let mut merged_any = true;
        let mut merged_at_least_once = false;
        while merged_any {
            merged_any = false;
            'outer: for left_index in 0..self.free_rects.len() {
                for right_index in (left_index + 1)..self.free_rects.len() {
                    if let Some(merged) = merge_adjacent_rects(
                        self.free_rects[left_index],
                        self.free_rects[right_index],
                        self.kerf_width,
                    ) {
                        self.free_rects[left_index] = merged;
                        self.free_rects.swap_remove(right_index);
                        merged_any = true;
                        merged_at_least_once = true;
                        break 'outer;
                    }
                }
            }
        }

        merged_at_least_once
    }

    fn merge_free_rects_and_invalidate_slicing_tree(&mut self) {
        if self.merge_free_rects() {
            self.slicing_tree = None;
        }
    }

    fn into_solution_sheet(self) -> SolutionSheet {
        SolutionSheet {
            stock_id: self.stock.stock_id,
            width: self.stock.width,
            length: self.stock.length,
            placed_pieces: self.placed_pieces,
            waste: self.free_rects,
            cutting_guide: self.slicing_tree.as_ref().map(GuideSliceNode::from),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GuillotineSliceNode {
    Cut {
        cut: GuideCut,
        first: Box<GuillotineSliceNode>,
        second: Box<GuillotineSliceNode>,
    },
    Leaf {
        rect: Rect,
        kind: GuillotineLeafKind,
    },
}

impl GuillotineSliceNode {
    fn cut(cut: GuideCut, first: Self, second: Self) -> Self {
        Self::Cut {
            cut,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    fn free(rect: Rect) -> Self {
        Self::Leaf {
            rect,
            kind: GuillotineLeafKind::Free,
        }
    }

    fn cut_piece(rect: Rect, cut: &CutInstance) -> Self {
        Self::Leaf {
            rect,
            kind: GuillotineLeafKind::CutPiece {
                cut_id: cut.cut_id,
                instance: cut.instance,
            },
        }
    }

    fn replace_free_leaf(&mut self, target: Rect, replacement: Self) -> bool {
        match self {
            Self::Cut { first, second, .. } => {
                first.replace_free_leaf(target, replacement.clone())
                    || second.replace_free_leaf(target, replacement)
            }
            Self::Leaf {
                rect,
                kind: GuillotineLeafKind::Free,
            } if *rect == target => {
                *self = replacement;
                true
            }
            Self::Leaf { .. } => false,
        }
    }

    #[cfg(test)]
    fn preorder_cuts(&self) -> Vec<GuideCut> {
        match self {
            Self::Cut { cut, first, second } => {
                let mut cuts = vec![*cut];
                cuts.extend(first.preorder_cuts());
                cuts.extend(second.preorder_cuts());
                cuts
            }
            Self::Leaf { .. } => Vec::new(),
        }
    }

    #[cfg(test)]
    fn free_leaf_rects(&self) -> Vec<Rect> {
        match self {
            Self::Cut { first, second, .. } => {
                let mut rects = first.free_leaf_rects();
                rects.extend(second.free_leaf_rects());
                rects
            }
            Self::Leaf {
                rect,
                kind: GuillotineLeafKind::Free,
            } => vec![*rect],
            Self::Leaf { .. } => Vec::new(),
        }
    }

    #[cfg(test)]
    fn cut_piece_leaf_records(&self) -> Vec<(PieceId, u32, Rect)> {
        match self {
            Self::Cut { first, second, .. } => {
                let mut records = first.cut_piece_leaf_records();
                records.extend(second.cut_piece_leaf_records());
                records
            }
            Self::Leaf {
                rect,
                kind: GuillotineLeafKind::CutPiece { cut_id, instance },
            } => vec![(*cut_id, *instance, *rect)],
            Self::Leaf { .. } => Vec::new(),
        }
    }
}

impl From<&GuillotineSliceNode> for GuideSliceNode {
    fn from(node: &GuillotineSliceNode) -> Self {
        match node {
            GuillotineSliceNode::Cut { cut, first, second } => GuideSliceNode::cut(
                *cut,
                GuideSliceNode::from(first.as_ref()),
                GuideSliceNode::from(second.as_ref()),
            ),
            GuillotineSliceNode::Leaf { rect, kind } => {
                GuideSliceNode::leaf(*rect, GuideLeafKind::from(*kind))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuillotineLeafKind {
    Free,
    CutPiece { cut_id: PieceId, instance: u32 },
}

impl From<GuillotineLeafKind> for GuideLeafKind {
    fn from(kind: GuillotineLeafKind) -> Self {
        match kind {
            GuillotineLeafKind::Free => Self::Waste,
            GuillotineLeafKind::CutPiece { cut_id, instance } => {
                Self::CutPiece { cut_id, instance }
            }
        }
    }
}

fn guillotine_insert_subtree(
    free_rect: Rect,
    placed: Rect,
    cut: &CutInstance,
    direction: GuillotineSplitDirection,
    kerf_width: u32,
) -> Option<GuillotineSliceNode> {
    debug_assert_eq!(placed.x, free_rect.x);
    debug_assert_eq!(placed.y, free_rect.y);

    match direction {
        GuillotineSplitDirection::Horizontal => {
            guillotine_horizontal_insert_subtree(free_rect, placed, cut, kerf_width)
        }
        GuillotineSplitDirection::Vertical => {
            guillotine_vertical_insert_subtree(free_rect, placed, cut, kerf_width)
        }
    }
}

fn guillotine_horizontal_insert_subtree(
    free_rect: Rect,
    placed: Rect,
    cut: &CutInstance,
    kerf_width: u32,
) -> Option<GuillotineSliceNode> {
    let remaining_length = free_rect.length - placed.length;
    let remaining_width = free_rect.width - placed.width;
    let mut top_node = GuillotineSliceNode::cut_piece(placed, cut);

    if remaining_width > kerf_width {
        let top_rect = Rect {
            x: free_rect.x,
            y: free_rect.y,
            width: free_rect.width,
            length: placed.length,
        };
        let right_rect = Rect {
            x: free_rect.x + placed.width + kerf_width,
            y: free_rect.y,
            width: remaining_width - kerf_width,
            length: placed.length,
        };
        let vertical_cut =
            GuideCut::new(top_rect, CutOrientation::Vertical, placed.width, kerf_width)?;
        top_node = GuillotineSliceNode::cut(
            vertical_cut,
            top_node,
            GuillotineSliceNode::free(right_rect),
        );
    }

    if remaining_length > kerf_width {
        let bottom_rect = Rect {
            x: free_rect.x,
            y: free_rect.y + placed.length + kerf_width,
            width: free_rect.width,
            length: remaining_length - kerf_width,
        };
        let horizontal_cut = GuideCut::new(
            free_rect,
            CutOrientation::Horizontal,
            placed.length,
            kerf_width,
        )?;
        return Some(GuillotineSliceNode::cut(
            horizontal_cut,
            top_node,
            GuillotineSliceNode::free(bottom_rect),
        ));
    }

    Some(top_node)
}

fn guillotine_vertical_insert_subtree(
    free_rect: Rect,
    placed: Rect,
    cut: &CutInstance,
    kerf_width: u32,
) -> Option<GuillotineSliceNode> {
    let remaining_length = free_rect.length - placed.length;
    let remaining_width = free_rect.width - placed.width;
    let mut left_node = GuillotineSliceNode::cut_piece(placed, cut);

    if remaining_length > kerf_width {
        let left_rect = Rect {
            x: free_rect.x,
            y: free_rect.y,
            width: placed.width,
            length: free_rect.length,
        };
        let bottom_rect = Rect {
            x: free_rect.x,
            y: free_rect.y + placed.length + kerf_width,
            width: placed.width,
            length: remaining_length - kerf_width,
        };
        let horizontal_cut = GuideCut::new(
            left_rect,
            CutOrientation::Horizontal,
            placed.length,
            kerf_width,
        )?;
        left_node = GuillotineSliceNode::cut(
            horizontal_cut,
            left_node,
            GuillotineSliceNode::free(bottom_rect),
        );
    }

    if remaining_width > kerf_width {
        let right_rect = Rect {
            x: free_rect.x + placed.width + kerf_width,
            y: free_rect.y,
            width: remaining_width - kerf_width,
            length: free_rect.length,
        };
        let vertical_cut = GuideCut::new(
            free_rect,
            CutOrientation::Vertical,
            placed.width,
            kerf_width,
        )?;
        return Some(GuillotineSliceNode::cut(
            vertical_cut,
            left_node,
            GuillotineSliceNode::free(right_rect),
        ));
    }

    Some(left_node)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuillotineSplitDirection {
    Horizontal,
    Vertical,
}

fn split_direction(
    free_rect: Rect,
    placed: Rect,
    split: GuillotineSplitHeuristic,
) -> GuillotineSplitDirection {
    match split {
        GuillotineSplitHeuristic::Horizontal => GuillotineSplitDirection::Horizontal,
        GuillotineSplitHeuristic::LongerAxis => {
            if free_rect.width > free_rect.length {
                GuillotineSplitDirection::Vertical
            } else {
                GuillotineSplitDirection::Horizontal
            }
        }
        GuillotineSplitHeuristic::MaximizeArea => {
            let remaining_width = free_rect.width.saturating_sub(placed.width);
            let remaining_length = free_rect.length.saturating_sub(placed.length);
            if u64::from(placed.width) * u64::from(remaining_length)
                <= u64::from(remaining_width) * u64::from(placed.length)
            {
                GuillotineSplitDirection::Horizontal
            } else {
                GuillotineSplitDirection::Vertical
            }
        }
    }
}

fn free_rect_choice_score(
    free_rect: Rect,
    placed: Rect,
    choice: GuillotineRectChoice,
) -> (u64, u64, u64, u32, u32) {
    let area_waste = (u64::from(free_rect.width) * u64::from(free_rect.length))
        .saturating_sub(u64::from(placed.width) * u64::from(placed.length));
    let remaining_width = free_rect.width.saturating_sub(placed.width);
    let remaining_length = free_rect.length.saturating_sub(placed.length);
    let short_side = remaining_width.min(remaining_length);
    let long_side = remaining_width.max(remaining_length);

    match choice {
        GuillotineRectChoice::Area => (
            area_waste,
            u64::from(short_side),
            u64::from(long_side),
            free_rect.y,
            free_rect.x,
        ),
        GuillotineRectChoice::ShortSide => (
            u64::from(short_side),
            u64::from(long_side),
            area_waste,
            free_rect.y,
            free_rect.x,
        ),
        GuillotineRectChoice::LongSide => (
            u64::from(long_side),
            u64::from(short_side),
            area_waste,
            free_rect.y,
            free_rect.x,
        ),
    }
}

fn merge_adjacent_rects(left: Rect, right: Rect, kerf_width: u32) -> Option<Rect> {
    if left.x == right.x && left.width == right.width {
        if left.y + left.length + kerf_width == right.y {
            return Some(Rect {
                x: left.x,
                y: left.y,
                width: left.width,
                length: left.length + kerf_width + right.length,
            });
        }
        if right.y + right.length + kerf_width == left.y {
            return Some(Rect {
                x: right.x,
                y: right.y,
                width: right.width,
                length: right.length + kerf_width + left.length,
            });
        }
    }

    if left.y == right.y && left.length == right.length {
        if left.x + left.width + kerf_width == right.x {
            return Some(Rect {
                x: left.x,
                y: left.y,
                width: left.width + kerf_width + right.width,
                length: left.length,
            });
        }
        if right.x + right.width + kerf_width == left.x {
            return Some(Rect {
                x: right.x,
                y: right.y,
                width: right.width + kerf_width + left.width,
                length: right.length,
            });
        }
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NestedHeuristic {
    rect_choice: NestedRectChoice,
    rotation: RotationPreference,
}

impl NestedHeuristic {
    const fn new(rect_choice: NestedRectChoice, rotation: RotationPreference) -> Self {
        Self {
            rect_choice,
            rotation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestedRectChoice {
    Area,
    ShortSide,
    LongSide,
    BottomLeft,
    ContactPoint,
}

struct NestedMaxRectsBackend;

impl PackingBackend for NestedMaxRectsBackend {
    type Bin = NestedSheet;
    type Heuristic = NestedHeuristic;

    fn default_heuristic() -> Self::Heuristic {
        NestedHeuristic::new(NestedRectChoice::Area, RotationPreference::PreferUpright)
    }

    fn heuristic_variants() -> Vec<Self::Heuristic> {
        vec![
            Self::default_heuristic(),
            NestedHeuristic::new(
                NestedRectChoice::ShortSide,
                RotationPreference::PreferUpright,
            ),
            NestedHeuristic::new(
                NestedRectChoice::LongSide,
                RotationPreference::PreferUpright,
            ),
            NestedHeuristic::new(
                NestedRectChoice::BottomLeft,
                RotationPreference::PreferUpright,
            ),
            NestedHeuristic::new(
                NestedRectChoice::ContactPoint,
                RotationPreference::PreferUpright,
            ),
            NestedHeuristic::new(NestedRectChoice::Area, RotationPreference::PreferRotated),
            NestedHeuristic::new(
                NestedRectChoice::ShortSide,
                RotationPreference::PreferRotated,
            ),
            NestedHeuristic::new(
                NestedRectChoice::LongSide,
                RotationPreference::PreferRotated,
            ),
            NestedHeuristic::new(
                NestedRectChoice::BottomLeft,
                RotationPreference::PreferRotated,
            ),
            NestedHeuristic::new(
                NestedRectChoice::ContactPoint,
                RotationPreference::PreferRotated,
            ),
        ]
    }

    fn new_bin(stock: StockInstance, kerf_width: u32) -> Self::Bin {
        NestedSheet::new(stock, kerf_width)
    }

    fn insert(bin: &mut Self::Bin, cut: &CutInstance, heuristic: Self::Heuristic) -> bool {
        bin.try_insert(cut, heuristic)
    }

    fn clone_stock_instance(bin: &Self::Bin) -> StockInstance {
        bin.stock.clone()
    }

    fn placed_cut_keys(bin: &Self::Bin) -> Vec<CutInstanceKey> {
        bin.placed_pieces.iter().map(CutInstanceKey::from).collect()
    }

    fn stock_area(bin: &Self::Bin) -> u64 {
        u64::from(bin.stock.width) * u64::from(bin.stock.length)
    }

    fn fitness(bin: &Self::Bin) -> f64 {
        let stock_area = Self::stock_area(bin);
        if stock_area == 0 {
            return 0.0;
        }

        let placed_area = bin
            .placed_pieces
            .iter()
            .map(|piece| u64::from(piece.rect.width) * u64::from(piece.rect.length))
            .sum::<u64>();

        area_as_fitness_weight(placed_area) / area_as_fitness_weight(stock_area)
    }

    fn into_solution_sheet(bin: Self::Bin) -> SolutionSheet {
        bin.into_solution_sheet()
    }
}

#[derive(Debug, Clone)]
struct NestedSheet {
    stock: StockInstance,
    kerf_width: u32,
    free_rects: Vec<Rect>,
    placed_pieces: Vec<PlacedPiece>,
}

impl NestedSheet {
    fn new(stock: StockInstance, kerf_width: u32) -> Self {
        Self {
            free_rects: vec![Rect {
                x: 0,
                y: 0,
                width: stock.width,
                length: stock.length,
            }],
            stock,
            kerf_width,
            placed_pieces: Vec::new(),
        }
    }

    fn try_insert(&mut self, cut: &CutInstance, heuristic: NestedHeuristic) -> bool {
        let Some(fit) = self.find_placement(cut, heuristic) else {
            return false;
        };

        self.split_free_rects(fit.rect);
        self.placed_pieces.push(PlacedPiece {
            cut_id: cut.cut_id,
            instance: cut.instance,
            rect: fit.rect,
            pattern: fit.pattern,
            rotated: fit.rotated,
        });
        true
    }

    fn find_placement(&self, cut: &CutInstance, heuristic: NestedHeuristic) -> Option<FitResult> {
        self.free_rects
            .iter()
            .copied()
            .flat_map(|free_rect| {
                fit_cut_in_rects(cut, self.stock.pattern, free_rect, heuristic.rotation)
                    .into_iter()
                    .map(move |fit| (free_rect, fit))
            })
            .min_by_key(|(free_rect, fit)| {
                self.nested_rect_choice_score(
                    *free_rect,
                    fit.rect,
                    fit.rotated,
                    heuristic.rect_choice,
                )
            })
            .map(|(_, fit)| fit)
    }

    fn nested_rect_choice_score(
        &self,
        free_rect: Rect,
        placed: Rect,
        rotated: bool,
        choice: NestedRectChoice,
    ) -> (u64, u64, u64, u64, u64, bool) {
        let area_waste = (u64::from(free_rect.width) * u64::from(free_rect.length))
            .saturating_sub(u64::from(placed.width) * u64::from(placed.length));
        let remaining_width = free_rect.width.saturating_sub(placed.width);
        let remaining_length = free_rect.length.saturating_sub(placed.length);
        let short_side = u64::from(remaining_width.min(remaining_length));
        let long_side = u64::from(remaining_width.max(remaining_length));
        let y = u64::from(free_rect.y);
        let x = u64::from(free_rect.x);

        match choice {
            NestedRectChoice::Area => (area_waste, short_side, long_side, y, x, rotated),
            NestedRectChoice::ShortSide => (short_side, long_side, area_waste, y, x, rotated),
            NestedRectChoice::LongSide => (long_side, short_side, area_waste, y, x, rotated),
            NestedRectChoice::BottomLeft => {
                let top_edge = u64::from(placed.y) + u64::from(placed.length);
                (top_edge, x, area_waste, short_side, long_side, rotated)
            }
            NestedRectChoice::ContactPoint => {
                let contact_score = self.contact_point_score(placed);
                (
                    u64::MAX - contact_score,
                    y,
                    x,
                    area_waste,
                    short_side,
                    rotated,
                )
            }
        }
    }

    fn contact_point_score(&self, rect: Rect) -> u64 {
        let mut score = 0;

        if rect.x == 0 || rect_right(rect) == u64::from(self.stock.width) {
            score += u64::from(rect.length);
        }

        if rect.y == 0 || rect_bottom(rect) == u64::from(self.stock.length) {
            score += u64::from(rect.width);
        }

        for placed in &self.placed_pieces {
            let placed_rect = placed.rect;
            if placed_rect.x == rect.x.saturating_add(rect.width)
                || placed_rect.x.saturating_add(placed_rect.width) == rect.x
            {
                score += common_interval_length(
                    placed_rect.y,
                    placed_rect.y.saturating_add(placed_rect.length),
                    rect.y,
                    rect.y.saturating_add(rect.length),
                );
            }

            if placed_rect.y == rect.y.saturating_add(rect.length)
                || placed_rect.y.saturating_add(placed_rect.length) == rect.y
            {
                score += common_interval_length(
                    placed_rect.x,
                    placed_rect.x.saturating_add(placed_rect.width),
                    rect.x,
                    rect.x.saturating_add(rect.width),
                );
            }
        }

        score
    }

    fn split_free_rects(&mut self, placed: Rect) {
        let occupied =
            expand_rect_for_kerf(placed, self.stock.width, self.stock.length, self.kerf_width);
        for index in (0..self.free_rects.len()).rev() {
            let free_rect = self.free_rects[index];
            if !rects_intersect(free_rect, occupied) {
                continue;
            }

            self.free_rects.swap_remove(index);
            self.free_rects
                .extend(split_free_rect_around_rect(free_rect, occupied));
        }
        prune_free_rects(&mut self.free_rects);
    }

    fn into_solution_sheet(self) -> SolutionSheet {
        SolutionSheet {
            stock_id: self.stock.stock_id,
            width: self.stock.width,
            length: self.stock.length,
            placed_pieces: self.placed_pieces,
            waste: make_free_rects_disjoint(self.free_rects),
            cutting_guide: None,
        }
    }
}

fn rects_intersect(left: Rect, right: Rect) -> bool {
    left.x < right.x + right.width
        && left.x + left.width > right.x
        && left.y < right.y + right.length
        && left.y + left.length > right.y
}

fn rect_contains(outer: Rect, inner: Rect) -> bool {
    u64::from(outer.x) <= u64::from(inner.x)
        && u64::from(outer.y) <= u64::from(inner.y)
        && rect_right(outer) >= rect_right(inner)
        && rect_bottom(outer) >= rect_bottom(inner)
}

fn rect_right(rect: Rect) -> u64 {
    u64::from(rect.x) + u64::from(rect.width)
}

fn rect_bottom(rect: Rect) -> u64 {
    u64::from(rect.y) + u64::from(rect.length)
}

fn common_interval_length(start1: u32, end1: u32, start2: u32, end2: u32) -> u64 {
    if end1 <= start2 || end2 <= start1 {
        0
    } else {
        u64::from(end1.min(end2) - start1.max(start2))
    }
}

fn prune_free_rects(free_rects: &mut Vec<Rect>) {
    let mut keep = vec![true; free_rects.len()];

    for inner_index in 0..free_rects.len() {
        for outer_index in 0..free_rects.len() {
            if inner_index == outer_index {
                continue;
            }

            let inner = free_rects[inner_index];
            let outer = free_rects[outer_index];

            if outer == inner && outer_index > inner_index {
                continue;
            }

            if rect_contains(outer, inner) {
                keep[inner_index] = false;
                break;
            }
        }
    }

    let pruned = free_rects
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, rect)| keep[index].then_some(rect))
        .collect();
    *free_rects = pruned;
}

fn make_free_rects_disjoint(mut free_rects: Vec<Rect>) -> Vec<Rect> {
    prune_free_rects(&mut free_rects);

    let mut accepted = Vec::new();
    for free_rect in free_rects {
        let mut remaining = vec![free_rect];

        for accepted_rect in &accepted {
            remaining = remaining
                .into_iter()
                .flat_map(|rect| subtract_rect(rect, *accepted_rect))
                .collect();

            if remaining.is_empty() {
                break;
            }
        }

        accepted.extend(remaining);
        prune_free_rects(&mut accepted);
    }

    prune_free_rects(&mut accepted);
    accepted
}

fn subtract_rect(free_rect: Rect, occupied: Rect) -> Vec<Rect> {
    if !rects_intersect(free_rect, occupied) {
        return vec![free_rect];
    }

    let free_right = free_rect.x + free_rect.width;
    let free_bottom = free_rect.y + free_rect.length;
    let occupied_right = occupied.x + occupied.width;
    let occupied_bottom = occupied.y + occupied.length;
    let intersection_left = free_rect.x.max(occupied.x);
    let intersection_top = free_rect.y.max(occupied.y);
    let intersection_right = free_right.min(occupied_right);
    let intersection_bottom = free_bottom.min(occupied_bottom);

    let mut remaining = Vec::new();

    if intersection_top > free_rect.y {
        remaining.push(Rect {
            x: free_rect.x,
            y: free_rect.y,
            width: free_rect.width,
            length: intersection_top - free_rect.y,
        });
    }

    if intersection_bottom < free_bottom {
        remaining.push(Rect {
            x: free_rect.x,
            y: intersection_bottom,
            width: free_rect.width,
            length: free_bottom - intersection_bottom,
        });
    }

    if intersection_left > free_rect.x {
        remaining.push(Rect {
            x: free_rect.x,
            y: intersection_top,
            width: intersection_left - free_rect.x,
            length: intersection_bottom - intersection_top,
        });
    }

    if intersection_right < free_right {
        remaining.push(Rect {
            x: intersection_right,
            y: intersection_top,
            width: free_right - intersection_right,
            length: intersection_bottom - intersection_top,
        });
    }

    remaining
}

fn expand_rect_for_kerf(rect: Rect, stock_width: u32, stock_length: u32, kerf_width: u32) -> Rect {
    let x = rect.x.saturating_sub(kerf_width);
    let y = rect.y.saturating_sub(kerf_width);
    let right = rect
        .x
        .saturating_add(rect.width)
        .saturating_add(kerf_width)
        .min(stock_width);
    let bottom = rect
        .y
        .saturating_add(rect.length)
        .saturating_add(kerf_width)
        .min(stock_length);

    Rect {
        x,
        y,
        width: right - x,
        length: bottom - y,
    }
}

fn split_free_rect_around_rect(free_rect: Rect, placed: Rect) -> Vec<Rect> {
    let mut split_rects = Vec::new();
    let free_right = free_rect.x + free_rect.width;
    let free_bottom = free_rect.y + free_rect.length;
    let placed_right = placed.x + placed.width;
    let placed_bottom = placed.y + placed.length;

    if placed.y > free_rect.y {
        split_rects.push(Rect {
            x: free_rect.x,
            y: free_rect.y,
            width: free_rect.width,
            length: placed.y - free_rect.y,
        });
    }

    if placed_bottom < free_bottom {
        split_rects.push(Rect {
            x: free_rect.x,
            y: placed_bottom,
            width: free_rect.width,
            length: free_bottom - placed_bottom,
        });
    }

    if placed.x > free_rect.x {
        split_rects.push(Rect {
            x: free_rect.x,
            y: free_rect.y,
            width: placed.x - free_rect.x,
            length: free_rect.length,
        });
    }

    if placed_right < free_right {
        split_rects.push(Rect {
            x: placed_right,
            y: free_rect.y,
            width: free_right - placed_right,
            length: free_rect.length,
        });
    }

    split_rects
}

#[derive(Debug, Clone, Copy)]
struct FitResult {
    rect: Rect,
    pattern: PatternDirection,
    rotated: bool,
}

fn fit_cut_in_rect(
    cut: &CutInstance,
    stock_pattern: PatternDirection,
    free_rect: Rect,
) -> Option<FitResult> {
    fit_cut_in_rects(
        cut,
        stock_pattern,
        free_rect,
        RotationPreference::PreferUpright,
    )
    .into_iter()
    .next()
}

fn fit_cut_in_rects(
    cut: &CutInstance,
    stock_pattern: PatternDirection,
    free_rect: Rect,
    rotation: RotationPreference,
) -> Vec<FitResult> {
    let mut upright = None;
    if pattern_matches(cut.pattern, stock_pattern)
        && cut.width <= free_rect.width
        && cut.length <= free_rect.length
    {
        upright = Some(FitResult {
            rect: Rect {
                x: free_rect.x,
                y: free_rect.y,
                width: cut.width,
                length: cut.length,
            },
            pattern: cut.pattern,
            rotated: false,
        });
    }

    let mut rotated = None;
    let rotated_pattern = rotate_pattern(cut.pattern);
    if cut.can_rotate
        && (cut.width != cut.length || rotated_pattern != cut.pattern)
        && pattern_matches(rotated_pattern, stock_pattern)
        && cut.length <= free_rect.width
        && cut.width <= free_rect.length
    {
        rotated = Some(FitResult {
            rect: Rect {
                x: free_rect.x,
                y: free_rect.y,
                width: cut.length,
                length: cut.width,
            },
            pattern: rotated_pattern,
            rotated: true,
        });
    }

    match rotation {
        RotationPreference::PreferUpright => [upright, rotated].into_iter().flatten().collect(),
        RotationPreference::PreferRotated => [rotated, upright].into_iter().flatten().collect(),
    }
}

fn pattern_matches(cut_pattern: PatternDirection, stock_pattern: PatternDirection) -> bool {
    cut_pattern == PatternDirection::None
        || stock_pattern == PatternDirection::None
        || cut_pattern == stock_pattern
}

fn rotate_pattern(pattern: PatternDirection) -> PatternDirection {
    match pattern {
        PatternDirection::None => PatternDirection::None,
        PatternDirection::ParallelToWidth => PatternDirection::ParallelToLength,
        PatternDirection::ParallelToLength => PatternDirection::ParallelToWidth,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizerEffort {
    #[default]
    Fast,
    Balanced,
    Thorough,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OptimizerConfig {
    pub effort: OptimizerEffort,
}

impl OptimizerConfig {
    #[must_use]
    pub fn new(effort: OptimizerEffort) -> Self {
        Self { effort }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProblemInstance {
    stock: Vec<StockInstance>,
    cuts: Vec<CutInstance>,
    kerf_width: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StockInstance {
    stock_id: PieceId,
    instance: u32,
    width: u32,
    length: u32,
    pattern: PatternDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CutInstance {
    cut_id: PieceId,
    instance: u32,
    width: u32,
    length: u32,
    pattern: PatternDirection,
    can_rotate: bool,
}

fn expand_project(project: &Project) -> Result<ProblemInstance, OptimizeError> {
    let mut stock = Vec::new();
    let mut cuts = Vec::new();

    for stock_piece in &project.stock_pieces {
        if stock_piece.width == 0 || stock_piece.length == 0 {
            return Err(OptimizeError::InvalidProject(format!(
                "stock piece {:?} has zero width or length",
                stock_piece.id
            )));
        }

        let quantity = stock_piece.quantity.ok_or_else(|| {
            OptimizeError::InvalidProject(format!(
                "stock piece {:?} has infinite quantity; this optimizer stage supports finite stock only",
                stock_piece.id
            ))
        })?;

        for instance in 0..quantity {
            stock.push(StockInstance {
                stock_id: stock_piece.id,
                instance,
                width: stock_piece.width,
                length: stock_piece.length,
                pattern: stock_piece.pattern,
            });
        }
    }

    for cut_piece in &project.cut_pieces {
        if cut_piece.width == 0 || cut_piece.length == 0 {
            return Err(OptimizeError::InvalidProject(format!(
                "cut piece {:?} has zero width or length",
                cut_piece.id
            )));
        }

        if cut_piece.quantity == 0 {
            return Err(OptimizeError::InvalidProject(format!(
                "cut piece {:?} has zero quantity",
                cut_piece.id
            )));
        }

        for instance in 0..cut_piece.quantity {
            cuts.push(CutInstance {
                cut_id: cut_piece.id,
                instance,
                width: cut_piece.width,
                length: cut_piece.length,
                pattern: cut_piece.pattern,
                can_rotate: cut_piece.can_rotate,
            });
        }
    }

    if !cuts.is_empty() && stock.is_empty() {
        return Err(OptimizeError::InvalidProject(
            "project contains cut pieces but no finite stock instances".to_string(),
        ));
    }

    // Stock input order is UI/import detail, not problem semantics. Canonicalize it before
    // first-fit candidate construction so a project file's row order cannot decide feasibility.
    stock.sort_by(compare_stock_instances_for_first_fit);

    Ok(ProblemInstance {
        stock,
        cuts,
        kerf_width: project.settings.kerf_width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CutPiece, CutSettings, StockPiece, Unit};

    fn optimize_first_fit<B>(instance: ProblemInstance) -> Result<Solution, OptimizeError>
    where
        B: PackingBackend,
    {
        let candidate = first_fit_candidate::<B>(instance);
        if candidate.is_valid() {
            Ok(candidate.into_solution(LayoutKind::Guillotine))
        } else {
            Err(OptimizeError::NoSolution)
        }
    }

    fn optimize_population<B>(
        instance: ProblemInstance,
        config: &InitialPopulationConfig,
    ) -> Result<Solution, OptimizeError>
    where
        B: PackingBackend,
    {
        optimize_population_pipeline::<B>(
            &instance,
            &PopulationPipelineConfig {
                initial: config.clone(),
                ..PopulationPipelineConfig::default()
            },
            LayoutKind::Guillotine,
        )
    }

    fn first_fit_candidate<B>(instance: ProblemInstance) -> Candidate<B>
    where
        B: PackingBackend,
    {
        let ProblemInstance {
            stock,
            mut cuts,
            kerf_width,
        } = instance;
        let cut_catalog = CutCatalog::new(cuts.clone());
        cuts.sort_by(compare_cut_instances_for_first_fit);

        let heuristic = B::default_heuristic();
        let mut candidate = Candidate::new(StockInventory::new(stock), cut_catalog);

        for cut in &cuts {
            candidate.place_cut_first_fit(cut, kerf_width, heuristic);
        }

        candidate
    }

    fn project(stock_pieces: Vec<StockPiece>, cut_pieces: Vec<CutPiece>) -> Project {
        Project {
            name: "test".to_string(),
            stock_pieces,
            cut_pieces,
            settings: CutSettings {
                unit: Unit::Millimeter,
                kerf_width: 3,
                layout: LayoutKind::Guillotine,
            },
        }
    }

    fn nested_project(stock_pieces: Vec<StockPiece>, cut_pieces: Vec<CutPiece>) -> Project {
        let mut project = project(stock_pieces, cut_pieces);
        project.settings.layout = LayoutKind::Nested;
        project
    }

    fn stock_piece(id: u64, quantity: Option<u32>) -> StockPiece {
        StockPiece {
            id: PieceId(id),
            width: 1000,
            length: 2000,
            quantity,
            pattern: PatternDirection::None,
        }
    }

    fn cut_piece(id: u64, quantity: u32) -> CutPiece {
        CutPiece {
            id: PieceId(id),
            label: format!("cut-{id}"),
            width: 100,
            length: 200,
            quantity,
            pattern: PatternDirection::ParallelToWidth,
            can_rotate: false,
        }
    }

    fn stock_with_size(id: u64, width: u32, length: u32, quantity: u32) -> StockPiece {
        StockPiece {
            id: PieceId(id),
            width,
            length,
            quantity: Some(quantity),
            pattern: PatternDirection::None,
        }
    }

    fn cut_with_size(id: u64, width: u32, length: u32, quantity: u32) -> CutPiece {
        CutPiece {
            id: PieceId(id),
            label: format!("cut-{id}"),
            width,
            length,
            quantity,
            pattern: PatternDirection::None,
            can_rotate: false,
        }
    }

    fn guillotine_sheet_with_size(width: u32, length: u32, kerf_width: u32) -> GuillotineSheet {
        GuillotineSheet::new(
            StockInstance {
                stock_id: PieceId(1),
                instance: 0,
                width,
                length,
                pattern: PatternDirection::None,
            },
            kerf_width,
        )
    }

    fn nested_sheet_with_free_rects(free_rects: Vec<Rect>) -> NestedSheet {
        let mut sheet = NestedSheet::new(
            StockInstance {
                stock_id: PieceId(1),
                instance: 0,
                width: 300,
                length: 300,
                pattern: PatternDirection::None,
            },
            0,
        );
        sheet.free_rects = free_rects;
        sheet
    }

    fn cut_instance_with_size(width: u32, length: u32) -> CutInstance {
        CutInstance {
            cut_id: PieceId(10),
            instance: 0,
            width,
            length,
            pattern: PatternDirection::None,
            can_rotate: false,
        }
    }

    fn cut_instance_with_id(id: u64, width: u32, length: u32) -> CutInstance {
        CutInstance {
            cut_id: PieceId(id),
            instance: 0,
            width,
            length,
            pattern: PatternDirection::None,
            can_rotate: false,
        }
    }

    fn sorted_rects(mut rects: Vec<Rect>) -> Vec<Rect> {
        rects.sort_by_key(|rect| (rect.y, rect.x, rect.width, rect.length));
        rects
    }

    fn sorted_cut_piece_leaf_records(
        mut records: Vec<(PieceId, u32, Rect)>,
    ) -> Vec<(PieceId, u32, Rect)> {
        records.sort_by_key(|(cut_id, instance, rect)| {
            (cut_id.0, *instance, rect.y, rect.x, rect.width, rect.length)
        });
        records
    }

    fn placed_piece_records(placed_pieces: &[PlacedPiece]) -> Vec<(PieceId, u32, Rect)> {
        sorted_cut_piece_leaf_records(
            placed_pieces
                .iter()
                .map(|piece| (piece.cut_id, piece.instance, piece.rect))
                .collect(),
        )
    }

    fn render_cut_piece_leaf_records(node: &GuideSliceNode) -> Vec<(PieceId, u32, Rect)> {
        match node {
            GuideSliceNode::Cut { first, second, .. } => {
                let mut records = render_cut_piece_leaf_records(first);
                records.extend(render_cut_piece_leaf_records(second));
                records
            }
            GuideSliceNode::Leaf {
                rect,
                kind: GuideLeafKind::CutPiece { cut_id, instance },
            } => vec![(*cut_id, *instance, *rect)],
            GuideSliceNode::Leaf { .. } => Vec::new(),
        }
    }

    fn render_waste_leaf_rects(node: &GuideSliceNode) -> Vec<Rect> {
        match node {
            GuideSliceNode::Cut { first, second, .. } => {
                let mut rects = render_waste_leaf_rects(first);
                rects.extend(render_waste_leaf_rects(second));
                rects
            }
            GuideSliceNode::Leaf {
                rect,
                kind: GuideLeafKind::Waste,
            } => vec![*rect],
            GuideSliceNode::Leaf { .. } => Vec::new(),
        }
    }

    fn assert_slicing_tree_geometry(node: &GuillotineSliceNode) -> Rect {
        match node {
            GuillotineSliceNode::Cut { cut, first, second } => {
                let work_rect = cut.work_rect();
                let first_rect = assert_slicing_tree_geometry(first);
                let second_rect = assert_slicing_tree_geometry(second);

                match cut.orientation() {
                    CutOrientation::Horizontal => {
                        assert_eq!(
                            first_rect,
                            Rect {
                                x: work_rect.x,
                                y: work_rect.y,
                                width: work_rect.width,
                                length: cut.offset(),
                            }
                        );
                        assert_eq!(
                            second_rect,
                            Rect {
                                x: work_rect.x,
                                y: work_rect.y + cut.offset() + cut.kerf_width(),
                                width: work_rect.width,
                                length: work_rect.length - cut.offset() - cut.kerf_width(),
                            }
                        );
                    }
                    CutOrientation::Vertical => {
                        assert_eq!(
                            first_rect,
                            Rect {
                                x: work_rect.x,
                                y: work_rect.y,
                                width: cut.offset(),
                                length: work_rect.length,
                            }
                        );
                        assert_eq!(
                            second_rect,
                            Rect {
                                x: work_rect.x + cut.offset() + cut.kerf_width(),
                                y: work_rect.y,
                                width: work_rect.width - cut.offset() - cut.kerf_width(),
                                length: work_rect.length,
                            }
                        );
                    }
                }

                work_rect
            }
            GuillotineSliceNode::Leaf { rect, .. } => {
                assert!(rect.width > 0);
                assert!(rect.length > 0);
                *rect
            }
        }
    }

    fn crossover_project_instance() -> ProblemInstance {
        expand_project(&project(
            vec![stock_with_size(1, 100, 100, 2)],
            vec![cut_with_size(10, 100, 100, 1), cut_with_size(11, 50, 50, 1)],
        ))
        .expect("project should expand")
    }

    fn first_fit_crossover_parent(
        instance: &ProblemInstance,
    ) -> Candidate<BaselineGuillotineBackend> {
        first_fit_candidate::<BaselineGuillotineBackend>(instance.clone())
    }

    fn compactable_two_sheet_candidate(
        instance: &ProblemInstance,
    ) -> Candidate<BaselineGuillotineBackend> {
        let mut first_bin =
            BaselineGuillotineBackend::new_bin(instance.stock[0].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut first_bin,
            &instance.cuts[0],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut second_bin =
            BaselineGuillotineBackend::new_bin(instance.stock[1].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut second_bin,
            &instance.cuts[1],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(Vec::new()),
            CutCatalog::new(instance.cuts.clone()),
        );
        candidate.bins = vec![first_bin, second_bin];
        candidate
    }

    fn stock_with_pattern(
        id: u64,
        width: u32,
        length: u32,
        pattern: PatternDirection,
    ) -> StockPiece {
        StockPiece {
            id: PieceId(id),
            width,
            length,
            quantity: Some(1),
            pattern,
        }
    }

    fn cut_with_pattern(
        id: u64,
        width: u32,
        length: u32,
        pattern: PatternDirection,
        can_rotate: bool,
    ) -> CutPiece {
        CutPiece {
            id: PieceId(id),
            label: format!("cut-{id}"),
            width,
            length,
            quantity: 1,
            pattern,
            can_rotate,
        }
    }

    fn assert_solution_within_bounds_and_non_overlapping(solution: &Solution) {
        for sheet in &solution.sheets {
            for (index, placed) in sheet.placed_pieces.iter().enumerate() {
                assert!(
                    placed.rect.x + placed.rect.width <= sheet.width,
                    "placed piece {index} exceeds sheet width"
                );
                assert!(
                    placed.rect.y + placed.rect.length <= sheet.length,
                    "placed piece {index} exceeds sheet length"
                );

                for other in sheet.placed_pieces.iter().skip(index + 1) {
                    assert!(
                        !rects_overlap(placed.rect, other.rect),
                        "placed pieces overlap: {:?} and {:?}",
                        placed.rect,
                        other.rect
                    );
                }
            }
        }
    }

    fn rects_overlap(left: Rect, right: Rect) -> bool {
        left.x < right.x + right.width
            && left.x + left.width > right.x
            && left.y < right.y + right.length
            && left.y + left.length > right.y
    }

    fn population_signature<B>(population: &[Candidate<B>]) -> Vec<(bool, Vec<CutInstanceKey>)>
    where
        B: PackingBackend,
    {
        population
            .iter()
            .map(|candidate| (candidate.is_valid(), candidate.placed_cut_keys()))
            .collect()
    }

    #[test]
    fn expands_quantities_into_stable_instances() {
        let project = project(
            vec![stock_piece(1, Some(2))],
            vec![cut_piece(10, 3), cut_piece(11, 1)],
        );

        let instance = expand_project(&project).expect("project should expand");

        assert_eq!(instance.kerf_width, 3);
        assert_eq!(
            instance
                .stock
                .iter()
                .map(|stock| (stock.stock_id, stock.instance))
                .collect::<Vec<_>>(),
            vec![(PieceId(1), 0), (PieceId(1), 1)]
        );
        assert_eq!(
            instance
                .cuts
                .iter()
                .map(|cut| (cut.cut_id, cut.instance))
                .collect::<Vec<_>>(),
            vec![
                (PieceId(10), 0),
                (PieceId(10), 1),
                (PieceId(10), 2),
                (PieceId(11), 0),
            ]
        );
    }

    #[test]
    fn rejects_infinite_stock_quantity_for_now() {
        let project = project(vec![stock_piece(1, None)], vec![cut_piece(10, 1)]);

        let error = expand_project(&project).expect_err("infinite stock is not supported yet");

        assert_eq!(
            error,
            OptimizeError::InvalidProject(
                "stock piece PieceId(1) has infinite quantity; this optimizer stage supports finite stock only"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_zero_dimensions() {
        let mut bad_stock = stock_piece(1, Some(1));
        bad_stock.width = 0;
        let error = expand_project(&project(vec![bad_stock], vec![cut_piece(10, 1)]))
            .expect_err("zero stock width is invalid");
        assert!(
            matches!(error, OptimizeError::InvalidProject(message) if message.contains("stock piece"))
        );

        let mut bad_cut = cut_piece(10, 1);
        bad_cut.length = 0;
        let error = expand_project(&project(vec![stock_piece(1, Some(1))], vec![bad_cut]))
            .expect_err("zero cut length is invalid");
        assert!(
            matches!(error, OptimizeError::InvalidProject(message) if message.contains("cut piece"))
        );
    }

    #[test]
    fn rejects_zero_cut_quantity() {
        let error = expand_project(&project(
            vec![stock_piece(1, Some(1))],
            vec![cut_piece(10, 0)],
        ))
        .expect_err("zero cut quantity is invalid");

        assert_eq!(
            error,
            OptimizeError::InvalidProject("cut piece PieceId(10) has zero quantity".to_string())
        );
    }

    #[test]
    fn rejects_cuts_without_finite_stock_instances() {
        let error = expand_project(&project(
            vec![stock_piece(1, Some(0))],
            vec![cut_piece(10, 1)],
        ))
        .expect_err("cuts need at least one finite stock instance");

        assert_eq!(
            error,
            OptimizeError::InvalidProject(
                "project contains cut pieces but no finite stock instances".to_string()
            )
        );
    }

    #[test]
    fn allows_empty_cut_list_without_stock() {
        let instance = expand_project(&project(Vec::new(), Vec::new()))
            .expect("empty cut list is a valid empty problem");

        assert!(instance.stock.is_empty());
        assert!(instance.cuts.is_empty());
        assert_eq!(instance.kerf_width, 3);
    }

    #[test]
    fn baseline_returns_empty_perfect_solution_for_empty_cut_list() {
        let solution = BaselineOptimizer
            .optimize(&project(Vec::new(), Vec::new()))
            .expect("empty cut list should optimize");

        assert!(solution.sheets.is_empty());
        assert_eq!(solution.fitness, Some(1.0));
    }

    #[test]
    fn baseline_places_exact_fit() {
        let mut project = project(
            vec![stock_with_size(1, 100, 200, 1)],
            vec![cut_with_size(10, 100, 200, 1)],
        );
        project.settings.kerf_width = 3;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("exact fit should optimize");

        assert_eq!(solution.fitness, Some(1.0));
        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].waste, Vec::new());
        assert_eq!(
            solution.sheets[0].placed_pieces[0].rect,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 200,
            }
        );
    }

    #[test]
    fn baseline_places_adjacent_pieces_when_kerf_fits() {
        let mut project = project(
            vec![stock_with_size(1, 103, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 3;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("two pieces plus kerf should fit");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 2);
        assert_eq!(
            solution.sheets[0]
                .placed_pieces
                .iter()
                .map(|piece| piece.rect)
                .collect::<Vec<_>>(),
            vec![
                Rect {
                    x: 0,
                    y: 0,
                    width: 50,
                    length: 50,
                },
                Rect {
                    x: 53,
                    y: 0,
                    width: 50,
                    length: 50,
                },
            ]
        );
    }

    #[test]
    fn baseline_returns_no_solution_when_kerf_prevents_second_piece() {
        let mut project = project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("kerf should prevent the second adjacent piece");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn baseline_returns_no_solution_for_cut_larger_than_stock() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 101, 100, 1)],
        );

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("oversized cut should not fit");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn nested_layout_is_accepted_by_public_optimizer() {
        let project = nested_project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 50, 50, 1)],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested layout should be optimized");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 1);
    }

    #[test]
    fn nested_places_exact_fit() {
        let project = nested_project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 100, 100, 1)],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested exact fit should produce a solution");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(
            solution.sheets[0].placed_pieces[0].rect,
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            }
        );
        assert!(solution.sheets[0].waste.is_empty());
    }

    #[test]
    fn nested_rejects_cut_larger_than_stock() {
        let project = nested_project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 101, 100, 1)],
        );

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("oversized nested cut should not fit");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn nested_solution_places_pieces_within_bounds_without_overlap() {
        let mut project = nested_project(
            vec![stock_with_size(1, 120, 120, 1)],
            vec![cut_with_size(10, 50, 50, 4)],
        );
        project.settings.kerf_width = 1;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested should place all pieces");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 4);
        assert_solution_within_bounds_and_non_overlapping(&solution);
    }

    #[test]
    fn nested_split_respects_kerf_without_moving_placed_rect() {
        let mut project = nested_project(
            vec![stock_with_size(1, 101, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("one kerf gap should allow two exact-height pieces");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 2);
        assert_eq!(
            solution.sheets[0]
                .placed_pieces
                .iter()
                .map(|piece| piece.rect)
                .collect::<Vec<_>>(),
            vec![
                Rect {
                    x: 0,
                    y: 0,
                    width: 50,
                    length: 50,
                },
                Rect {
                    x: 51,
                    y: 0,
                    width: 50,
                    length: 50,
                },
            ]
        );
    }

    #[test]
    fn nested_kerf_prevents_too_tight_adjacent_placement() {
        let mut project = nested_project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("kerf should prevent two exact-width pieces in 100 mm stock");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn nested_zero_kerf_allows_exact_tiling() {
        let mut project = nested_project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 0;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("zero kerf should allow exact tiling");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 2);
        assert_solution_within_bounds_and_non_overlapping(&solution);
    }

    #[test]
    fn nested_split_keeps_free_rects_within_sheet_bounds() {
        let mut project = nested_project(
            vec![stock_with_size(1, 120, 100, 1)],
            vec![
                cut_with_size(10, 40, 40, 1),
                cut_with_size(20, 30, 50, 1),
                cut_with_size(30, 20, 20, 1),
            ],
        );
        project.settings.kerf_width = 2;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested should produce a solution");

        let sheet = &solution.sheets[0];
        assert!(sheet.waste.iter().all(|waste| {
            waste.x + waste.width <= sheet.width && waste.y + waste.length <= sheet.length
        }));
    }

    #[test]
    fn nested_prunes_free_rect_contained_by_larger_rect() {
        let mut free_rects = vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            },
            Rect {
                x: 20,
                y: 20,
                width: 30,
                length: 30,
            },
            Rect {
                x: 120,
                y: 0,
                width: 20,
                length: 20,
            },
        ];

        prune_free_rects(&mut free_rects);

        assert_eq!(
            free_rects,
            vec![
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    length: 100,
                },
                Rect {
                    x: 120,
                    y: 0,
                    width: 20,
                    length: 20,
                },
            ]
        );
    }

    #[test]
    fn nested_pruning_keeps_non_contained_overlapping_rects() {
        let mut free_rects = vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 50,
            },
            Rect {
                x: 50,
                y: 0,
                width: 100,
                length: 50,
            },
        ];

        prune_free_rects(&mut free_rects);

        assert_eq!(
            free_rects,
            vec![
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    length: 50,
                },
                Rect {
                    x: 50,
                    y: 0,
                    width: 100,
                    length: 50,
                },
            ]
        );
    }

    #[test]
    fn nested_pruning_is_deterministic_and_stable() {
        let first = Rect {
            x: 0,
            y: 0,
            width: 100,
            length: 40,
        };
        let second = Rect {
            x: 110,
            y: 0,
            width: 80,
            length: 80,
        };
        let contained = Rect {
            x: 120,
            y: 10,
            width: 20,
            length: 20,
        };
        let third = Rect {
            x: 0,
            y: 50,
            width: 40,
            length: 40,
        };
        let mut free_rects = vec![first, first, second, contained, third];

        prune_free_rects(&mut free_rects);

        assert_eq!(free_rects, vec![first, second, third]);
    }

    #[test]
    fn nested_output_waste_rects_are_disjoint() {
        let project = nested_project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 50, 50, 1)],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested should produce a solution");

        let waste = &solution.sheets[0].waste;
        for (index, left) in waste.iter().enumerate() {
            for right in waste.iter().skip(index + 1) {
                assert!(
                    !rects_overlap(*left, *right),
                    "waste rects should be disjoint: {left:?} and {right:?}"
                );
            }
        }
    }

    #[test]
    fn nested_waste_rects_do_not_overlap_placed_pieces() {
        let mut project = nested_project(
            vec![stock_with_size(1, 120, 100, 1)],
            vec![
                cut_with_size(10, 40, 40, 1),
                cut_with_size(20, 30, 50, 1),
                cut_with_size(30, 20, 20, 1),
            ],
        );
        project.settings.kerf_width = 2;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested should produce a solution");

        let sheet = &solution.sheets[0];
        for waste in &sheet.waste {
            for placed in &sheet.placed_pieces {
                assert!(
                    !rects_overlap(*waste, placed.rect),
                    "waste should not overlap placed piece: {waste:?} and {:?}",
                    placed.rect
                );
            }
        }
    }

    #[test]
    fn nested_waste_rects_stay_within_sheet_bounds() {
        let mut project = nested_project(
            vec![stock_with_size(1, 120, 100, 1)],
            vec![
                cut_with_size(10, 40, 40, 1),
                cut_with_size(20, 30, 50, 1),
                cut_with_size(30, 20, 20, 1),
            ],
        );
        project.settings.kerf_width = 2;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("nested should produce a solution");

        let sheet = &solution.sheets[0];
        assert!(sheet.waste.iter().all(|waste| {
            waste.x + waste.width <= sheet.width && waste.y + waste.length <= sheet.length
        }));
    }

    #[test]
    fn nested_waste_normalizer_splits_overlapping_free_rects_without_kerf() {
        let normalized = make_free_rects_disjoint(vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            },
            Rect {
                x: 50,
                y: 0,
                width: 100,
                length: 100,
            },
        ]);

        assert_eq!(
            normalized,
            vec![
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    length: 100,
                },
                Rect {
                    x: 100,
                    y: 0,
                    width: 50,
                    length: 100,
                },
            ]
        );
        for (index, left) in normalized.iter().enumerate() {
            for right in normalized.iter().skip(index + 1) {
                assert!(!rects_overlap(*left, *right));
            }
        }
    }

    #[test]
    fn nested_area_choice_prefers_smallest_area_waste() {
        let sheet = nested_sheet_with_free_rects(vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            },
            Rect {
                x: 120,
                y: 0,
                width: 50,
                length: 50,
            },
        ]);
        let cut = cut_instance_with_size(40, 40);

        let fit = sheet
            .find_placement(
                &cut,
                NestedHeuristic::new(NestedRectChoice::Area, RotationPreference::PreferUpright),
            )
            .expect("cut should fit");

        assert_eq!(fit.rect.x, 120);
    }

    #[test]
    fn nested_short_side_choice_prefers_tighter_short_side() {
        let sheet = nested_sheet_with_free_rects(vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 45,
            },
            Rect {
                x: 110,
                y: 0,
                width: 65,
                length: 100,
            },
        ]);
        let cut = cut_instance_with_size(60, 40);

        let fit = sheet
            .find_placement(
                &cut,
                NestedHeuristic::new(
                    NestedRectChoice::ShortSide,
                    RotationPreference::PreferUpright,
                ),
            )
            .expect("cut should fit");

        assert_eq!(fit.rect.x, 0);
    }

    #[test]
    fn nested_long_side_choice_prefers_tighter_long_side() {
        let sheet = nested_sheet_with_free_rects(vec![
            Rect {
                x: 0,
                y: 0,
                width: 90,
                length: 90,
            },
            Rect {
                x: 100,
                y: 0,
                width: 120,
                length: 50,
            },
        ]);
        let cut = cut_instance_with_size(60, 40);

        let fit = sheet
            .find_placement(
                &cut,
                NestedHeuristic::new(
                    NestedRectChoice::LongSide,
                    RotationPreference::PreferUpright,
                ),
            )
            .expect("cut should fit");

        assert_eq!(fit.rect.x, 0);
    }

    #[test]
    fn nested_bottom_left_choice_prefers_lowest_top_edge() {
        let sheet = nested_sheet_with_free_rects(vec![
            Rect {
                x: 0,
                y: 10,
                width: 40,
                length: 40,
            },
            Rect {
                x: 50,
                y: 0,
                width: 40,
                length: 40,
            },
        ]);
        let cut = cut_instance_with_size(20, 20);

        let fit = sheet
            .find_placement(
                &cut,
                NestedHeuristic::new(
                    NestedRectChoice::BottomLeft,
                    RotationPreference::PreferUpright,
                ),
            )
            .expect("cut should fit");

        assert_eq!(fit.rect.x, 50);
        assert_eq!(fit.rect.y, 0);
    }

    #[test]
    fn nested_contact_point_choice_prefers_more_contact() {
        let mut sheet = nested_sheet_with_free_rects(vec![
            Rect {
                x: 100,
                y: 0,
                width: 40,
                length: 40,
            },
            Rect {
                x: 20,
                y: 0,
                width: 40,
                length: 40,
            },
        ]);
        sheet.placed_pieces.push(PlacedPiece {
            cut_id: PieceId(1),
            instance: 0,
            rect: Rect {
                x: 0,
                y: 0,
                width: 20,
                length: 20,
            },
            pattern: PatternDirection::None,
            rotated: false,
        });
        let cut = cut_instance_with_size(20, 20);

        let fit = sheet
            .find_placement(
                &cut,
                NestedHeuristic::new(
                    NestedRectChoice::ContactPoint,
                    RotationPreference::PreferUpright,
                ),
            )
            .expect("cut should fit");

        assert_eq!(fit.rect.x, 20);
        assert_eq!(fit.rect.y, 0);
    }

    #[test]
    fn nested_optimizer_is_deterministic_for_same_input() {
        let mut project = nested_project(
            vec![stock_with_size(1, 120, 100, 1)],
            vec![
                cut_with_size(10, 40, 50, 1),
                cut_with_size(20, 60, 40, 1),
                cut_with_size(30, 30, 30, 2),
            ],
        );
        project.settings.kerf_width = 1;

        let first = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Balanced))
            .expect("first nested run should produce a solution");
        let second = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Balanced))
            .expect("second nested run should produce a solution");

        assert_eq!(first, second);
    }

    #[test]
    fn nested_thorough_is_deterministic() {
        let mut project = nested_project(
            vec![stock_with_size(1, 180, 120, 1)],
            vec![
                cut_with_size(10, 70, 40, 1),
                cut_with_size(20, 60, 50, 1),
                cut_with_size(30, 30, 80, 1),
                cut_with_size(40, 25, 25, 3),
            ],
        );
        project.settings.kerf_width = 1;

        let first = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
            .expect("first thorough nested run should produce a solution");
        let second = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
            .expect("second thorough nested run should produce a solution");

        assert_eq!(first, second);
    }

    #[test]
    fn baseline_does_not_exceed_stock_quantity() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 60, 100, 2)],
        );

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("only one sheet is available");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn first_fit_candidate_keeps_remaining_stock_in_inventory() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 2)],
            vec![cut_with_size(10, 60, 100, 1)],
        );
        let instance = expand_project(&project).expect("project should expand");

        let candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);

        assert!(candidate.is_valid());
        assert_eq!(candidate.remaining_stock_count(), 1);
    }

    #[test]
    fn sorted_candidate_seed_matches_current_first_fit_candidate() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");

        let current = first_fit_candidate::<BaselineGuillotineBackend>(instance.clone());
        let seed = sorted_first_fit_candidate_seed::<BaselineGuillotineBackend>(&instance.cuts);
        let seeded = candidate_from_seed::<BaselineGuillotineBackend>(&instance, &seed)
            .expect("seed keys should resolve through the cut catalog");

        assert_eq!(seed.description, CandidateSeedDescription::SortedFirstFit);
        assert_eq!(
            seed.cut_order,
            vec![
                CutInstanceKey {
                    cut_id: PieceId(30),
                    instance: 0,
                },
                CutInstanceKey {
                    cut_id: PieceId(10),
                    instance: 0,
                },
                CutInstanceKey {
                    cut_id: PieceId(20),
                    instance: 0,
                },
            ]
        );
        assert_eq!(seeded.placed_cut_keys(), current.placed_cut_keys());
        assert_eq!(seeded.used_stock_keys(), current.used_stock_keys());
        assert_eq!(
            seeded.remaining_stock_count(),
            current.remaining_stock_count()
        );
        assert_eq!(
            seeded
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>(),
            current
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(seeded.is_valid(), current.is_valid());
    }

    #[test]
    fn initial_population_default_builds_sorted_first_fit_candidate() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let current = first_fit_candidate::<BaselineGuillotineBackend>(instance.clone());

        let population = initial_population::<BaselineGuillotineBackend>(
            &instance,
            &InitialPopulationConfig::default(),
        );

        assert_eq!(population.len(), 1);
        assert_eq!(population[0].placed_cut_keys(), current.placed_cut_keys());
        assert_eq!(population[0].used_stock_keys(), current.used_stock_keys());
        assert_eq!(
            population[0].remaining_stock_count(),
            current.remaining_stock_count()
        );
        assert_eq!(population[0].is_valid(), current.is_valid());
    }

    #[test]
    fn initial_population_builds_sorted_and_shuffled_candidates() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
                cut_with_size(40, 10, 90, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };

        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);

        assert_eq!(population.len(), 2);
        assert_eq!(
            population[0].placed_cut_keys(),
            sorted_cut_order(&instance.cuts)
        );
        assert_eq!(
            population[1].placed_cut_keys(),
            shuffle_cut_order(&instance.cuts, 1)
        );
        assert!(population.iter().all(Candidate::is_valid));
    }

    #[test]
    fn initial_population_builds_shuffled_only_candidate() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
                cut_with_size(40, 10, 90, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 2,
            include_sorted_first_fit: false,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };

        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);

        assert_eq!(population.len(), 1);
        assert_eq!(
            population[0].placed_cut_keys(),
            shuffle_cut_order(&instance.cuts, 2)
        );
        assert!(population[0].is_valid());
    }

    #[test]
    fn initial_population_can_hold_invalid_candidates_but_public_optimizer_returns_no_solution() {
        let mut project = project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };

        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);

        assert_eq!(population.len(), 2);
        assert!(population.iter().all(|candidate| !candidate.is_valid()));
        assert!(population
            .iter()
            .all(|candidate| candidate.placed_cut_keys().len() == 1));

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("public optimizer must reject invalid final candidates");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn basic_feasibility_prefilter_rejects_single_cut_that_fits_no_stock() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 101, 100, 1)],
        );
        let instance = expand_project(&project).expect("project should expand");

        let error = basic_feasibility_prefilter(&instance)
            .expect_err("oversized cut should be rejected before population construction");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn basic_feasibility_prefilter_allows_rotated_single_cut_that_can_fit_stock() {
        let project = project(
            vec![stock_with_size(1, 50, 100, 1)],
            vec![CutPiece {
                can_rotate: true,
                ..cut_with_size(10, 100, 50, 1)
            }],
        );
        let instance = expand_project(&project).expect("project should expand");

        assert_eq!(basic_feasibility_prefilter(&instance), Ok(()));
    }

    #[test]
    fn basic_feasibility_prefilter_rejects_pattern_mismatch_without_rotation() {
        let project = project(
            vec![stock_with_pattern(
                1,
                100,
                200,
                PatternDirection::ParallelToLength,
            )],
            vec![cut_with_pattern(
                10,
                100,
                200,
                PatternDirection::ParallelToWidth,
                false,
            )],
        );
        let instance = expand_project(&project).expect("project should expand");

        let error = basic_feasibility_prefilter(&instance)
            .expect_err("pattern mismatch should be rejected before population construction");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn basic_feasibility_prefilter_rejects_when_total_cut_area_exceeds_stock_area() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 100, 60, 2)],
        );
        let instance = expand_project(&project).expect("project should expand");

        assert!(instance.cuts.iter().all(|cut| instance
            .stock
            .iter()
            .any(|stock| cut_fits_stock(cut, stock))));
        assert_eq!(total_cut_area(&instance.cuts), 12_000);
        assert_eq!(total_stock_area(&instance.stock), 10_000);

        let error = basic_feasibility_prefilter(&instance)
            .expect_err("total cut area should not exceed total stock area");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn basic_feasibility_prefilter_allows_equal_total_area() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 100, 100, 1)],
        );
        let instance = expand_project(&project).expect("project should expand");

        assert_eq!(
            total_cut_area(&instance.cuts),
            total_stock_area(&instance.stock)
        );
        assert_eq!(basic_feasibility_prefilter(&instance), Ok(()));
    }

    #[test]
    fn basic_feasibility_prefilter_allows_patternless_cut_on_patterned_stock() {
        let project = project(
            vec![stock_with_pattern(
                1,
                100,
                200,
                PatternDirection::ParallelToLength,
            )],
            vec![cut_with_pattern(
                10,
                100,
                200,
                PatternDirection::None,
                false,
            )],
        );
        let instance = expand_project(&project).expect("project should expand");

        assert_eq!(basic_feasibility_prefilter(&instance), Ok(()));
    }

    #[test]
    fn basic_feasibility_prefilter_allows_patterned_cut_on_patternless_stock() {
        let project = project(
            vec![stock_with_pattern(1, 100, 200, PatternDirection::None)],
            vec![cut_with_pattern(
                10,
                100,
                200,
                PatternDirection::ParallelToWidth,
                false,
            )],
        );
        let instance = expand_project(&project).expect("project should expand");

        assert_eq!(basic_feasibility_prefilter(&instance), Ok(()));
    }

    #[test]
    fn crossover_population_disabled_preserves_population() {
        let mut project = project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);
        let before = population_signature(&population);

        let after = crossover_population(
            population,
            CrossoverConfig { enabled: false },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(population_signature(&after), before);
    }

    #[test]
    fn crossover_population_enabled_adds_child_from_donor_bin() {
        let instance = crossover_project_instance();
        let left = first_fit_crossover_parent(&instance);
        let right = first_fit_crossover_parent(&instance);

        let crossed = crossover_population(
            vec![left, right],
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(crossed.len(), 3);
        let child = &crossed[2];
        assert!(!child.is_valid());
        assert_eq!(child.remaining_stock_count(), 1);
        assert_eq!(
            child.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            }]
        );
        assert_eq!(
            child
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>(),
            vec![CutInstanceKey {
                cut_id: PieceId(11),
                instance: 0,
            }]
        );
        assert!(!child.has_duplicate_placed_cut_keys());
    }

    #[test]
    fn crossover_population_enabled_keeps_odd_parent_without_extra_child() {
        let instance = crossover_project_instance();
        let first = first_fit_crossover_parent(&instance);
        let second = first_fit_crossover_parent(&instance);
        let third = first_fit_crossover_parent(&instance);
        let third_signature = (third.is_valid(), third.placed_cut_keys());

        let crossed = crossover_population(
            vec![first, second, third],
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(crossed.len(), 4);
        assert_eq!(
            (crossed[2].is_valid(), crossed[2].placed_cut_keys()),
            third_signature
        );
        assert!(!crossed[3].is_valid());
    }

    #[test]
    fn crossover_population_skips_child_with_unresolved_donor_cut() {
        let stock_source_instance = crossover_project_instance();
        let donor_instance = expand_project(&project(
            vec![stock_with_size(1, 100, 100, 2)],
            vec![
                cut_with_size(99, 100, 100, 1),
                cut_with_size(100, 50, 50, 1),
            ],
        ))
        .expect("donor project should expand");
        assert!(stock_source_instance
            .cuts
            .iter()
            .all(|cut| cut.cut_id != PieceId(99)));
        assert!(donor_instance
            .cuts
            .iter()
            .any(|cut| cut.cut_id == PieceId(99)));
        let population = vec![
            first_fit_crossover_parent(&stock_source_instance),
            first_fit_crossover_parent(&donor_instance),
        ];
        let before = population_signature(&population);

        let crossed = crossover_population(
            population,
            CrossoverConfig { enabled: true },
            stock_source_instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(crossed.len(), 2);
        assert_eq!(population_signature(&crossed), before);
    }

    #[test]
    fn crossover_population_uses_best_filled_donor_bin() {
        let instance = crossover_project_instance();
        let stock_source = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(instance.stock.clone()),
            CutCatalog::new(instance.cuts.clone()),
        );

        let mut small_bin =
            BaselineGuillotineBackend::new_bin(instance.stock[0].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut small_bin,
            &instance.cuts[1],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut full_bin =
            BaselineGuillotineBackend::new_bin(instance.stock[1].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut full_bin,
            &instance.cuts[0],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut donor = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(Vec::new()),
            CutCatalog::new(instance.cuts.clone()),
        );
        donor.bins = vec![small_bin, full_bin];

        let crossed = crossover_population(
            vec![stock_source, donor],
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(crossed.len(), 3);
        let child = &crossed[2];
        assert_eq!(
            child.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            }]
        );
        assert_eq!(
            child.used_stock_keys(),
            vec![StockInstanceKey {
                stock_id: PieceId(1),
                instance: 1,
            }]
        );
        assert!(child.fitness() > crossed[1].fitness());
    }

    #[test]
    fn crossover_population_enabled_preserves_less_than_two_parents() {
        let instance = crossover_project_instance();

        let crossed_empty = crossover_population::<BaselineGuillotineBackend>(
            Vec::new(),
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        assert!(crossed_empty.is_empty());

        let single = first_fit_crossover_parent(&instance);
        let before = population_signature(std::slice::from_ref(&single));
        let crossed_single = crossover_population(
            vec![single],
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(crossed_single.len(), 1);
        assert_eq!(population_signature(&crossed_single), before);
    }

    #[test]
    fn crossover_child_can_be_repaired_without_exceeding_stock() {
        let instance = crossover_project_instance();
        let left = first_fit_crossover_parent(&instance);
        let right = first_fit_crossover_parent(&instance);
        let crossed = crossover_population(
            vec![left, right],
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        let repaired = repair_population(
            crossed,
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let child = &repaired[2];

        assert!(child.is_valid());
        assert_eq!(child.remaining_stock_count(), 0);
        assert_eq!(child.placed_cut_keys().len(), 2);
        assert!(!child.has_duplicate_placed_cut_keys());
    }

    #[test]
    fn crossover_repair_survival_can_select_repaired_child() {
        let instance = crossover_project_instance();
        let mut first_parent_full_stock =
            BaselineGuillotineBackend::new_bin(instance.stock[0].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut first_parent_full_stock,
            &instance.cuts[0],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut first_parent_partial_stock =
            BaselineGuillotineBackend::new_bin(instance.stock[1].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut first_parent_partial_stock,
            &instance.cuts[1],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut first_parent = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(Vec::new()),
            CutCatalog::new(instance.cuts.clone()),
        );
        first_parent.bins = vec![first_parent_full_stock, first_parent_partial_stock];
        first_parent.unused_cuts.push(instance.cuts[1].clone());

        let mut second_parent_full_stock =
            BaselineGuillotineBackend::new_bin(instance.stock[0].clone(), instance.kerf_width);
        assert!(BaselineGuillotineBackend::insert(
            &mut second_parent_full_stock,
            &instance.cuts[0],
            BaselineGuillotineBackend::default_heuristic()
        ));
        let mut second_parent = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(Vec::new()),
            CutCatalog::new(instance.cuts.clone()),
        );
        second_parent.bins.push(second_parent_full_stock);
        second_parent.unused_cuts.push(instance.cuts[1].clone());

        let crossed = crossover_population(
            vec![first_parent, second_parent],
            CrossoverConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        assert_eq!(crossed.len(), 3);

        let repaired = repair_population(
            crossed,
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let survivors = run_population_epochs(
            repaired,
            &PopulationConfig {
                epochs: 1,
                survivor_limit: None,
            },
        );
        assert_eq!(survivors.len(), 1);
        assert!(survivors.iter().all(Candidate::is_valid));

        let selected = select_best_valid_candidate(survivors)
            .expect("repaired crossover child should be selected");

        assert_eq!(
            selected.placed_cut_keys(),
            vec![
                CutInstanceKey {
                    cut_id: PieceId(10),
                    instance: 0,
                },
                CutInstanceKey {
                    cut_id: PieceId(11),
                    instance: 0,
                },
            ]
        );
        assert!(!selected.has_duplicate_placed_cut_keys());
        assert_eq!(selected.remaining_stock_count(), 0);
    }

    #[test]
    fn compact_population_can_reduce_used_sheets_after_repair() {
        let mut project = project(
            vec![stock_with_size(1, 100, 100, 2)],
            vec![cut_with_size(10, 50, 100, 1), cut_with_size(11, 50, 50, 1)],
        );
        project.settings.kerf_width = 0;
        let instance = expand_project(&project).expect("project should expand");
        let candidate = compactable_two_sheet_candidate(&instance);
        assert_eq!(
            candidate.score(),
            Some(CandidateScore {
                used_stock_count: 2,
                waste_area: 12_500,
                fitness: Some(0.375),
            })
        );

        let compacted = compact_population(vec![candidate], CompactionConfig { enabled: true });
        let repaired = repair_population(
            compacted,
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let compacted_candidate = &repaired[0];

        assert!(compacted_candidate.is_valid());
        assert_eq!(
            compacted_candidate
                .score()
                .map(|score| score.used_stock_count),
            Some(1)
        );
        assert_eq!(compacted_candidate.placed_cut_keys().len(), 2);
        assert!(!compacted_candidate.has_duplicate_placed_cut_keys());
    }

    #[test]
    fn generation_loop_runs_practical_stages_before_survival() {
        let mut project = project(
            vec![stock_with_size(1, 100, 100, 2)],
            vec![cut_with_size(10, 50, 100, 1), cut_with_size(11, 50, 50, 1)],
        );
        project.settings.kerf_width = 0;
        let instance = expand_project(&project).expect("project should expand");
        let survival_only = run_population_epochs(
            vec![compactable_two_sheet_candidate(&instance)],
            &PopulationConfig {
                epochs: 2,
                survivor_limit: Some(1),
            },
        );
        let mut staged_config = PopulationPipelineConfig::default();
        staged_config.crossover = CrossoverConfig { enabled: true };
        staged_config.repair = RepairConfig { enabled: true };
        staged_config.compaction = CompactionConfig { enabled: true };
        staged_config.population = PopulationConfig {
            epochs: 2,
            survivor_limit: Some(1),
        };

        let staged = run_population_generations(
            vec![compactable_two_sheet_candidate(&instance)],
            &staged_config,
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(survival_only.len(), 1);
        assert_eq!(
            survival_only[0].score().map(|score| score.used_stock_count),
            Some(2)
        );
        assert_eq!(staged.len(), 1);
        assert!(staged[0].is_valid());
        assert_eq!(
            staged[0].score().map(|score| score.used_stock_count),
            Some(1)
        );
    }

    #[test]
    fn repair_population_disabled_preserves_population() {
        let mut project = project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);
        let before = population_signature(&population);

        let after = repair_population(
            population,
            RepairConfig { enabled: false },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(population_signature(&after), before);
    }

    #[test]
    fn repair_population_enabled_can_make_candidate_valid_using_remaining_stock() {
        let instance = expand_project(&project(
            vec![stock_with_size(1, 50, 50, 2)],
            vec![cut_with_size(10, 50, 50, 1)],
        ))
        .expect("project should expand");
        let mut candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(instance.stock.clone()),
            CutCatalog::new(instance.cuts.clone()),
        );
        candidate.unused_cuts.push(instance.cuts[0].clone());

        let repaired = repair_population(
            vec![candidate],
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(repaired.len(), 1);
        assert!(repaired[0].is_valid());
        assert_eq!(repaired[0].remaining_stock_count(), 1);
        assert_eq!(
            repaired[0].placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            }]
        );
    }

    #[test]
    fn repair_population_failed_repair_does_not_duplicate_unused_cuts() {
        let instance = expand_project(&project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 101, 100, 1)],
        ))
        .expect("project should expand");
        let mut candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(instance.stock.clone()),
            CutCatalog::new(instance.cuts.clone()),
        );
        candidate.unused_cuts.push(instance.cuts[0].clone());

        let repaired_once = repair_population(
            vec![candidate],
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let repaired_twice = repair_population(
            repaired_once,
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(repaired_twice.len(), 1);
        assert!(!repaired_twice[0].is_valid());
        assert_eq!(
            repaired_twice[0]
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            }]
        );
    }

    #[test]
    fn repair_population_does_not_exceed_finite_stock() {
        let instance = expand_project(&project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 100, 100, 2)],
        ))
        .expect("project should expand");
        let mut candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(instance.stock.clone()),
            CutCatalog::new(instance.cuts.clone()),
        );
        candidate.unused_cuts.extend(instance.cuts.iter().cloned());

        let repaired = repair_population(
            vec![candidate],
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert_eq!(repaired.len(), 1);
        assert!(!repaired[0].is_valid());
        assert_eq!(repaired[0].remaining_stock_count(), 0);
        assert_eq!(repaired[0].placed_cut_keys().len(), 1);
        assert!(!repaired[0].has_duplicate_placed_cut_keys());
        assert_eq!(repaired[0].unused_cuts.len(), 1);
    }

    #[test]
    fn repair_population_selection_ignores_candidate_that_remains_invalid() {
        let instance = expand_project(&project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 101, 100, 1)],
        ))
        .expect("project should expand");
        let mut candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(instance.stock.clone()),
            CutCatalog::new(instance.cuts.clone()),
        );
        candidate.unused_cuts.push(instance.cuts[0].clone());

        let repaired = repair_population(
            vec![candidate],
            RepairConfig { enabled: true },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );

        assert!(select_best_valid_candidate(repaired).is_none());
    }

    #[test]
    fn repaired_candidates_can_survive_and_be_selected() {
        let repairable_instance = expand_project(&project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 100, 100, 1)],
        ))
        .expect("repairable project should expand");
        let mut repairable_candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(repairable_instance.stock.clone()),
            CutCatalog::new(repairable_instance.cuts.clone()),
        );
        repairable_candidate
            .unused_cuts
            .push(repairable_instance.cuts[0].clone());
        let low_fitness_candidate = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(20, 50, 50, 1)],
            ))
            .expect("low-fitness project should expand"),
        );
        let invalid_instance = expand_project(&project(
            vec![stock_with_size(3, 100, 100, 1)],
            vec![cut_with_size(30, 101, 100, 1)],
        ))
        .expect("invalid project should expand");
        let mut invalid_candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(invalid_instance.stock.clone()),
            CutCatalog::new(invalid_instance.cuts.clone()),
        );
        invalid_candidate
            .unused_cuts
            .push(invalid_instance.cuts[0].clone());

        let repaired = repair_population(
            vec![
                repairable_candidate,
                low_fitness_candidate,
                invalid_candidate,
            ],
            RepairConfig { enabled: true },
            repairable_instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let survivors = run_population_epochs(
            repaired,
            &PopulationConfig {
                epochs: 1,
                survivor_limit: None,
            },
        );
        assert_eq!(survivors.len(), 2);
        assert!(survivors.iter().all(Candidate::is_valid));

        let selected = select_best_valid_candidate(survivors)
            .expect("repaired valid candidate should be selected");

        assert_eq!(selected.fitness(), Some(1.0));
        assert_eq!(
            selected.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            }]
        );
    }

    #[test]
    fn run_population_epochs_zero_preserves_population() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
                cut_with_size(40, 10, 90, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);
        let before = population_signature(&population);

        let after = run_population_epochs(
            population,
            &PopulationConfig {
                epochs: 0,
                survivor_limit: None,
            },
        );

        assert_eq!(population_signature(&after), before);
    }

    #[test]
    fn run_population_epochs_positive_keeps_valid_candidates_by_descending_fitness() {
        let low_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(10, 50, 50, 1)],
            ))
            .expect("low-fitness project should expand"),
        );
        let invalid_high_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(20, 90, 100, 1), cut_with_size(21, 20, 100, 1)],
            ))
            .expect("invalid project should expand"),
        );
        let high_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(3, 100, 100, 1)],
                vec![cut_with_size(30, 100, 100, 1)],
            ))
            .expect("high-fitness project should expand"),
        );
        let equal_low_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(4, 100, 100, 1)],
                vec![cut_with_size(40, 50, 50, 1)],
            ))
            .expect("equal-fitness project should expand"),
        );

        assert!(low_fitness.is_valid());
        assert!(!invalid_high_fitness.is_valid());
        assert!(high_fitness.is_valid());
        assert!(equal_low_fitness.is_valid());

        let after = run_population_epochs(
            vec![
                low_fitness,
                invalid_high_fitness,
                high_fitness,
                equal_low_fitness,
            ],
            &PopulationConfig {
                epochs: 1,
                survivor_limit: None,
            },
        );

        assert_eq!(
            after
                .iter()
                .map(|candidate| candidate.placed_cut_keys()[0].cut_id)
                .collect::<Vec<_>>(),
            vec![PieceId(30), PieceId(10), PieceId(40)]
        );
        assert!(after.iter().all(Candidate::is_valid));
    }

    #[test]
    fn run_population_epochs_drops_invalid_candidates() {
        let mut project = project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let population = initial_population::<BaselineGuillotineBackend>(&instance, &config);

        let after = run_population_epochs(
            population,
            &PopulationConfig {
                epochs: 2,
                survivor_limit: None,
            },
        );

        assert!(after.is_empty());
    }

    #[test]
    fn run_population_epochs_applies_survivor_limit_after_sorting() {
        let low_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(10, 50, 50, 1)],
            ))
            .expect("low-fitness project should expand"),
        );
        let high_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(20, 100, 100, 1)],
            ))
            .expect("high-fitness project should expand"),
        );

        let after = run_population_epochs(
            vec![low_fitness, high_fitness],
            &PopulationConfig {
                epochs: 1,
                survivor_limit: Some(1),
            },
        );

        assert_eq!(after.len(), 1);
        assert_eq!(
            after[0].placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(20),
                instance: 0,
            }]
        );
    }

    #[test]
    fn run_population_epochs_survivor_limit_zero_leaves_no_selected_candidate() {
        let candidate = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(10, 50, 50, 1)],
            ))
            .expect("valid project should expand"),
        );

        let after = run_population_epochs(
            vec![candidate],
            &PopulationConfig {
                epochs: 1,
                survivor_limit: Some(0),
            },
        );

        assert!(after.is_empty());
        assert!(select_best_valid_candidate(after).is_none());
    }

    #[test]
    fn run_population_epochs_survivor_limit_preserves_stable_equal_fitness_order() {
        let first = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(30, 50, 50, 1)],
            ))
            .expect("first equal-fitness project should expand"),
        );
        let second = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(10, 50, 50, 1)],
            ))
            .expect("second equal-fitness project should expand"),
        );
        let third = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(3, 100, 100, 1)],
                vec![cut_with_size(20, 50, 50, 1)],
            ))
            .expect("third equal-fitness project should expand"),
        );

        let after = run_population_epochs(
            vec![first, second, third],
            &PopulationConfig {
                epochs: 1,
                survivor_limit: Some(2),
            },
        );

        assert_eq!(
            after
                .iter()
                .map(|candidate| candidate.placed_cut_keys()[0].cut_id)
                .collect::<Vec<_>>(),
            vec![PieceId(30), PieceId(10)]
        );
    }

    #[test]
    fn selection_after_survival_epochs_matches_selection_before_epochs() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
                cut_with_size(40, 10, 90, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let before = select_best_valid_candidate(initial_population::<BaselineGuillotineBackend>(
            &instance, &config,
        ))
        .expect("valid candidate should be selected");
        let after_population = run_population_epochs(
            initial_population::<BaselineGuillotineBackend>(&instance, &config),
            &PopulationConfig {
                epochs: 4,
                survivor_limit: None,
            },
        );
        let after = select_best_valid_candidate(after_population)
            .expect("valid candidate should still be selected");

        assert_eq!(after.fitness(), before.fitness());
        assert_eq!(after.placed_cut_keys(), before.placed_cut_keys());
    }

    #[test]
    fn optimize_population_returns_solution_for_best_valid_candidate() {
        let project = project(
            vec![stock_with_size(1, 100, 200, 1)],
            vec![cut_with_size(10, 100, 200, 1)],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig::default();

        let solution = optimize_population::<BaselineGuillotineBackend>(instance, &config)
            .expect("valid population should produce a solution");

        assert_eq!(solution.fitness, Some(1.0));
        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 1);
    }

    #[test]
    fn optimizer_config_balanced_can_improve_fast_effort() {
        let mut project = project(
            vec![stock_with_size(1, 103, 100, 2)],
            vec![cut_with_size(10, 60, 60, 1), cut_with_size(20, 40, 100, 1)],
        );
        project.settings.kerf_width = 3;

        let fast_solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Fast))
            .expect("fast optimizer should produce a solution");
        let balanced_solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Balanced))
            .expect("balanced optimizer should produce a solution");

        assert_eq!(fast_solution.sheets.len(), 2);
        assert_eq!(balanced_solution.sheets.len(), 1);
        assert!(balanced_solution.fitness > fast_solution.fitness);
    }

    #[test]
    fn optimizer_config_thorough_is_deterministic() {
        let mut project = project(
            vec![stock_with_size(1, 103, 100, 2)],
            vec![cut_with_size(10, 60, 60, 1), cut_with_size(20, 40, 100, 1)],
        );
        project.settings.kerf_width = 3;

        let first_solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
            .expect("thorough optimizer should produce a solution");
        let second_solution = BaselineOptimizer
            .optimize_with_config(&project, OptimizerConfig::new(OptimizerEffort::Thorough))
            .expect("thorough optimizer should reproduce the solution");

        assert_eq!(first_solution, second_solution);
    }

    #[test]
    fn genetic_default_pipeline_can_improve_sorted_first_fit() {
        let mut project = project(
            vec![stock_with_size(1, 103, 100, 2)],
            vec![cut_with_size(10, 60, 60, 1), cut_with_size(20, 40, 100, 1)],
        );
        project.settings.kerf_width = 3;
        let instance = expand_project(&project).expect("project should expand");

        let sorted_solution = optimize_first_fit::<BaselineGuillotineBackend>(instance.clone())
            .expect("sorted first-fit should produce a solution");
        let genetic_solution = optimize_population_pipeline::<BaselineGuillotineBackend>(
            &instance,
            &PopulationPipelineConfig::genetic_default(0),
            LayoutKind::Guillotine,
        )
        .expect("private genetic pipeline should produce a solution");

        assert_eq!(sorted_solution.sheets.len(), 2);
        assert_eq!(genetic_solution.sheets.len(), 1);
        assert!(genetic_solution.fitness > sorted_solution.fitness);
    }

    #[test]
    fn optimize_population_default_matches_current_first_fit_solution() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");

        let first_fit_solution = optimize_first_fit::<BaselineGuillotineBackend>(instance.clone())
            .expect("first-fit should produce a solution");
        let population_solution = optimize_population::<BaselineGuillotineBackend>(
            instance,
            &InitialPopulationConfig::default(),
        )
        .expect("default population should produce a solution");
        let baseline_solution = BaselineOptimizer
            .optimize(&project)
            .expect("baseline optimizer should produce a solution");

        assert_eq!(population_solution, first_fit_solution);
        assert_eq!(population_solution, baseline_solution);
    }

    #[test]
    fn optimize_population_matches_manual_epoch_zero_pipeline() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
                cut_with_size(40, 10, 90, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let manual_population = initial_population::<BaselineGuillotineBackend>(&instance, &config);
        let manual_population = crossover_population(
            manual_population,
            CrossoverConfig { enabled: false },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let manual_population = repair_population(
            manual_population,
            RepairConfig { enabled: false },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let manual_population =
            compact_population(manual_population, CompactionConfig { enabled: false });
        let manual_population = repair_population(
            manual_population,
            RepairConfig { enabled: false },
            instance.kerf_width,
            BaselineGuillotineBackend::default_heuristic(),
        );
        let manual_population = run_population_epochs(
            manual_population,
            &PopulationConfig {
                epochs: 0,
                survivor_limit: None,
            },
        );
        let manual_solution = select_best_valid_candidate(manual_population)
            .map(|candidate| candidate.into_solution(LayoutKind::Guillotine))
            .expect("manual epoch-zero pipeline should produce a solution");

        let optimized_solution =
            optimize_population::<BaselineGuillotineBackend>(instance, &config)
                .expect("population optimizer should produce a solution");

        assert_eq!(optimized_solution, manual_solution);
    }

    #[test]
    fn population_pipeline_with_initial_config_uses_default_private_stages() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let initial_config = InitialPopulationConfig {
            seed: 7,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };
        let pipeline_config = PopulationPipelineConfig {
            initial: initial_config.clone(),
            ..PopulationPipelineConfig::default()
        };

        assert_eq!(pipeline_config.initial, initial_config);
        assert_eq!(
            pipeline_config.crossover,
            CrossoverConfig { enabled: false }
        );
        assert_eq!(pipeline_config.repair, RepairConfig { enabled: false });
        assert_eq!(
            pipeline_config.compaction,
            CompactionConfig { enabled: false }
        );
        assert_eq!(
            pipeline_config.population,
            PopulationConfig {
                epochs: 0,
                survivor_limit: None,
            }
        );

        let optimized_solution = optimize_population::<BaselineGuillotineBackend>(
            instance.clone(),
            &pipeline_config.initial,
        )
        .expect("default population optimizer should produce a solution");
        let pipeline_solution = optimize_population_pipeline::<BaselineGuillotineBackend>(
            &instance,
            &pipeline_config,
            LayoutKind::Guillotine,
        )
        .expect("explicit private pipeline should produce a solution");

        assert_eq!(optimized_solution, pipeline_solution);
    }

    #[test]
    fn optimize_population_returns_no_solution_when_all_candidates_are_invalid() {
        let mut project = project(
            vec![stock_with_size(1, 100, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 1;
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: true,
            shuffled_candidate_count: 1,
            include_heuristic_variants: false,
            max_candidates: None,
        };

        let error = optimize_population::<BaselineGuillotineBackend>(instance, &config)
            .expect_err("invalid population should not produce a solution");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn select_best_valid_candidate_ignores_invalid_candidates() {
        let invalid_candidate = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(10, 90, 100, 1), cut_with_size(11, 20, 100, 1)],
            ))
            .expect("invalid project should expand"),
        );
        let valid_candidate = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(20, 50, 50, 1)],
            ))
            .expect("valid project should expand"),
        );

        assert!(!invalid_candidate.is_valid());
        assert_eq!(invalid_candidate.fitness(), Some(0.9));
        assert!(valid_candidate.is_valid());
        assert_eq!(valid_candidate.fitness(), Some(0.25));

        let selected = select_best_valid_candidate(vec![invalid_candidate, valid_candidate])
            .expect("valid candidate should be selected");

        assert_eq!(selected.fitness(), Some(0.25));
        assert_eq!(
            selected.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(20),
                instance: 0,
            }]
        );
    }

    #[test]
    fn select_best_valid_candidate_returns_none_when_all_candidates_are_invalid() {
        let first_invalid = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(10, 60, 100, 2)],
            ))
            .expect("first invalid project should expand"),
        );
        let second_invalid = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(20, 70, 100, 2)],
            ))
            .expect("second invalid project should expand"),
        );

        assert!(!first_invalid.is_valid());
        assert!(!second_invalid.is_valid());

        assert!(select_best_valid_candidate(vec![first_invalid, second_invalid]).is_none());
    }

    #[test]
    fn select_best_valid_candidate_prefers_fewer_used_sheets() {
        let one_sheet_lower_fill = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 200, 100, 1)],
                vec![cut_with_size(10, 100, 100, 1)],
            ))
            .expect("one-sheet project should expand"),
        );
        let two_sheets_perfect_fill = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 2)],
                vec![cut_with_size(20, 100, 100, 2)],
            ))
            .expect("two-sheet project should expand"),
        );

        assert!(one_sheet_lower_fill.is_valid());
        assert_eq!(
            one_sheet_lower_fill.score(),
            Some(CandidateScore {
                used_stock_count: 1,
                waste_area: 10_000,
                fitness: Some(0.5),
            })
        );
        assert!(two_sheets_perfect_fill.is_valid());
        assert_eq!(
            two_sheets_perfect_fill.score(),
            Some(CandidateScore {
                used_stock_count: 2,
                waste_area: 0,
                fitness: Some(1.0),
            })
        );

        let selected =
            select_best_valid_candidate(vec![two_sheets_perfect_fill, one_sheet_lower_fill])
                .expect("candidate using fewer sheets should be selected");

        assert_eq!(
            selected.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            }]
        );
    }

    #[test]
    fn select_best_valid_candidate_picks_highest_fitness_among_valid_candidates() {
        let low_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(1, 100, 100, 1)],
                vec![cut_with_size(10, 50, 50, 1)],
            ))
            .expect("low-fitness project should expand"),
        );
        let high_fitness = first_fit_candidate::<BaselineGuillotineBackend>(
            expand_project(&project(
                vec![stock_with_size(2, 100, 100, 1)],
                vec![cut_with_size(20, 100, 100, 1)],
            ))
            .expect("high-fitness project should expand"),
        );

        assert!(low_fitness.is_valid());
        assert_eq!(low_fitness.fitness(), Some(0.25));
        assert!(high_fitness.is_valid());
        assert_eq!(high_fitness.fitness(), Some(1.0));

        let selected = select_best_valid_candidate(vec![low_fitness, high_fitness])
            .expect("highest-fitness valid candidate should be selected");

        assert_eq!(selected.fitness(), Some(1.0));
        assert_eq!(
            selected.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(20),
                instance: 0,
            }]
        );
    }

    #[test]
    fn initial_candidate_seeds_respects_config_limits() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");

        let mut config = InitialPopulationConfig::default();
        let seeds = initial_candidate_seeds::<BaselineGuillotineBackend>(&instance.cuts, &config);
        assert_eq!(seeds.len(), 1);
        assert_eq!(
            seeds[0].description,
            CandidateSeedDescription::SortedFirstFit
        );

        config.include_shuffled_first_fit = true;
        config.seed = 1;
        let seeds = initial_candidate_seeds::<BaselineGuillotineBackend>(&instance.cuts, &config);
        assert_eq!(seeds.len(), 2);
        assert_eq!(
            seeds[0].description,
            CandidateSeedDescription::SortedFirstFit
        );
        assert_eq!(
            seeds[1].description,
            CandidateSeedDescription::ShuffledFirstFit { seed: 1, index: 0 }
        );
        assert_eq!(seeds[1].cut_order, shuffle_cut_order(&instance.cuts, 1));

        config.max_candidates = Some(1);
        let seeds = initial_candidate_seeds::<BaselineGuillotineBackend>(&instance.cuts, &config);
        assert_eq!(seeds.len(), 1);
        assert_eq!(
            seeds[0].description,
            CandidateSeedDescription::SortedFirstFit
        );

        config.max_candidates = Some(0);
        assert!(
            initial_candidate_seeds::<BaselineGuillotineBackend>(&instance.cuts, &config)
                .is_empty()
        );

        config.max_candidates = None;
        config.include_sorted_first_fit = false;
        let seeds = initial_candidate_seeds::<BaselineGuillotineBackend>(&instance.cuts, &config);
        assert_eq!(seeds.len(), 1);
        assert_eq!(
            seeds[0].description,
            CandidateSeedDescription::ShuffledFirstFit { seed: 1, index: 0 }
        );
    }

    #[test]
    fn initial_candidate_seeds_include_guillotine_heuristic_variants_when_enabled() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = InitialPopulationConfig {
            seed: 1,
            include_sorted_first_fit: true,
            include_shuffled_first_fit: false,
            shuffled_candidate_count: 0,
            include_heuristic_variants: true,
            max_candidates: None,
        };

        let seeds = initial_candidate_seeds::<BaselineGuillotineBackend>(&instance.cuts, &config);

        assert_eq!(seeds.len(), 5);
        assert_eq!(
            seeds[0].description,
            CandidateSeedDescription::SortedFirstFit
        );
        assert_eq!(
            seeds[1].description,
            CandidateSeedDescription::HeuristicVariant { index: 1 }
        );
        assert_eq!(
            seeds[1].heuristic,
            BaselineGuillotineHeuristic::new(
                GuillotineRectChoice::LongSide,
                GuillotineSplitHeuristic::LongerAxis,
                RotationPreference::PreferUpright,
            )
        );
        assert!(seeds
            .iter()
            .all(|seed| seed.cut_order == sorted_cut_order(&instance.cuts)));
    }

    #[test]
    fn guillotine_balanced_config_adds_variants_without_truncating_shuffles() {
        let base = PopulationPipelineConfig::from_optimizer_config(OptimizerConfig::new(
            OptimizerEffort::Balanced,
        ));
        assert!(!base.initial.include_heuristic_variants);
        assert_eq!(base.initial.shuffled_candidate_count, 8);
        assert_eq!(base.initial.max_candidates, Some(9));

        let guillotine =
            guillotine_pipeline_config(OptimizerConfig::new(OptimizerEffort::Balanced));
        assert!(guillotine.initial.include_heuristic_variants);
        assert_eq!(guillotine.initial.shuffled_candidate_count, 8);
        assert_eq!(guillotine.initial.max_candidates, Some(13));

        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let seeds = initial_candidate_seeds::<BaselineGuillotineBackend>(
            &instance.cuts,
            &guillotine.initial,
        );

        assert_eq!(seeds.len(), 13);
        assert_eq!(
            seeds
                .iter()
                .filter(|seed| matches!(
                    seed.description,
                    CandidateSeedDescription::ShuffledFirstFit { .. }
                ))
                .count(),
            8
        );
    }

    #[test]
    fn nested_heuristic_variants_are_included_for_thorough_seeds() {
        let project = nested_project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(20, 30, 40, 1),
                cut_with_size(10, 50, 10, 1),
                cut_with_size(30, 50, 20, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let config = PopulationPipelineConfig::thorough_default(1).initial;

        let seeds = initial_candidate_seeds::<NestedMaxRectsBackend>(&instance.cuts, &config);

        assert_eq!(seeds.len(), 29);
        assert_eq!(
            seeds[0].description,
            CandidateSeedDescription::SortedFirstFit
        );
        assert_eq!(
            seeds[1..10]
                .iter()
                .map(|seed| seed.description.clone())
                .collect::<Vec<_>>(),
            (1..10)
                .map(|index| CandidateSeedDescription::HeuristicVariant { index })
                .collect::<Vec<_>>()
        );
        assert_eq!(
            seeds[0].heuristic,
            NestedHeuristic::new(NestedRectChoice::Area, RotationPreference::PreferUpright)
        );
        assert_eq!(
            seeds[1].heuristic,
            NestedHeuristic::new(
                NestedRectChoice::ShortSide,
                RotationPreference::PreferUpright
            )
        );
        assert_eq!(
            seeds[4].heuristic,
            NestedHeuristic::new(
                NestedRectChoice::ContactPoint,
                RotationPreference::PreferUpright,
            )
        );
        assert_eq!(
            seeds[5].heuristic,
            NestedHeuristic::new(NestedRectChoice::Area, RotationPreference::PreferRotated)
        );
        assert_eq!(
            seeds[9].heuristic,
            NestedHeuristic::new(
                NestedRectChoice::ContactPoint,
                RotationPreference::PreferRotated,
            )
        );
        assert!(seeds[..10]
            .iter()
            .all(|seed| seed.cut_order == sorted_cut_order(&instance.cuts)));
        assert!(matches!(
            seeds[10].description,
            CandidateSeedDescription::ShuffledFirstFit { .. }
        ));
    }

    #[test]
    fn guillotine_longer_axis_split_keeps_full_length_side_rect_on_wide_free_rect() {
        let mut sheet = GuillotineSheet::new(
            StockInstance {
                stock_id: PieceId(1),
                instance: 0,
                width: 2440,
                length: 1220,
                pattern: PatternDirection::None,
            },
            1,
        );
        sheet.free_rects.clear();

        sheet.split_free_rect(
            Rect {
                x: 0,
                y: 0,
                width: 2440,
                length: 1220,
            },
            Rect {
                x: 0,
                y: 0,
                width: 234,
                length: 344,
            },
            GuillotineSplitHeuristic::LongerAxis,
        );

        assert_eq!(
            sheet.free_rects,
            vec![
                Rect {
                    x: 0,
                    y: 345,
                    width: 234,
                    length: 875,
                },
                Rect {
                    x: 235,
                    y: 0,
                    width: 2205,
                    length: 1220,
                },
            ]
        );
    }

    #[test]
    fn guillotine_insert_builds_preorder_slicing_subtree_for_horizontal_split() {
        let mut sheet = guillotine_sheet_with_size(100, 80, 3);
        let cut = cut_instance_with_size(60, 50);

        assert!(sheet.try_insert(&cut, BaselineGuillotineBackend::default_heuristic()));

        let tree = sheet
            .slicing_tree
            .as_ref()
            .expect("simple insert should keep a slicing tree");
        let cuts = tree.preorder_cuts();

        assert_eq!(
            cuts.iter()
                .map(|cut| (
                    cut.work_rect(),
                    cut.orientation(),
                    cut.offset(),
                    cut.kerf_width()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    Rect {
                        x: 0,
                        y: 0,
                        width: 100,
                        length: 80,
                    },
                    CutOrientation::Horizontal,
                    50,
                    3,
                ),
                (
                    Rect {
                        x: 0,
                        y: 0,
                        width: 100,
                        length: 50,
                    },
                    CutOrientation::Vertical,
                    60,
                    3,
                ),
            ]
        );
        assert_eq!(
            sorted_rects(tree.free_leaf_rects()),
            sorted_rects(sheet.free_rects.clone())
        );
    }

    #[test]
    fn guillotine_insert_builds_preorder_slicing_subtree_for_vertical_split() {
        let mut sheet = guillotine_sheet_with_size(100, 60, 3);
        let cut = cut_instance_with_size(40, 30);
        let heuristic = BaselineGuillotineHeuristic::new(
            GuillotineRectChoice::Area,
            GuillotineSplitHeuristic::LongerAxis,
            RotationPreference::PreferUpright,
        );

        assert!(sheet.try_insert(&cut, heuristic));

        let tree = sheet
            .slicing_tree
            .as_ref()
            .expect("simple insert should keep a slicing tree");
        let cuts = tree.preorder_cuts();

        assert_eq!(
            cuts.iter()
                .map(|cut| (
                    cut.work_rect(),
                    cut.orientation(),
                    cut.offset(),
                    cut.kerf_width()
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    Rect {
                        x: 0,
                        y: 0,
                        width: 100,
                        length: 60,
                    },
                    CutOrientation::Vertical,
                    40,
                    3,
                ),
                (
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        length: 60,
                    },
                    CutOrientation::Horizontal,
                    30,
                    3,
                ),
            ]
        );
        assert_eq!(
            sorted_rects(tree.free_leaf_rects()),
            sorted_rects(sheet.free_rects.clone())
        );
    }

    #[test]
    fn guillotine_exact_fit_replaces_free_leaf_without_cut_events() {
        let mut sheet = guillotine_sheet_with_size(50, 50, 3);
        let cut = cut_instance_with_size(50, 50);

        assert!(sheet.try_insert(&cut, BaselineGuillotineBackend::default_heuristic()));

        let tree = sheet
            .slicing_tree
            .as_ref()
            .expect("exact fit should remain representable as a tree");
        assert_eq!(tree.preorder_cuts(), Vec::new());
        assert_eq!(tree.free_leaf_rects(), Vec::new());
        assert_eq!(sheet.free_rects, Vec::new());
    }

    #[test]
    fn guillotine_slicing_tree_cuts_partition_their_work_rects() {
        let mut sheet = guillotine_sheet_with_size(100, 80, 3);
        let first_cut = cut_instance_with_id(10, 60, 50);
        let second_cut = cut_instance_with_id(11, 20, 20);

        assert!(sheet.try_insert(&first_cut, BaselineGuillotineBackend::default_heuristic()));
        assert!(sheet.try_insert(&second_cut, BaselineGuillotineBackend::default_heuristic()));

        let tree = sheet
            .slicing_tree
            .as_ref()
            .expect("unmerged inserts should keep the slicing tree representable");

        assert_eq!(
            assert_slicing_tree_geometry(tree),
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 80,
            }
        );
    }

    #[test]
    fn guillotine_slicing_tree_converts_to_render_slice_node() {
        let mut sheet = guillotine_sheet_with_size(100, 80, 3);
        let first_cut = cut_instance_with_id(10, 60, 50);
        let second_cut = cut_instance_with_id(11, 20, 20);

        assert!(sheet.try_insert(&first_cut, BaselineGuillotineBackend::default_heuristic()));
        assert!(sheet.try_insert(&second_cut, BaselineGuillotineBackend::default_heuristic()));

        let tree = sheet
            .slicing_tree
            .as_ref()
            .expect("unmerged inserts should keep the slicing tree representable");
        let render_tree = GuideSliceNode::from(tree);

        assert_eq!(
            render_tree.preorder_cuts().copied().collect::<Vec<_>>(),
            tree.preorder_cuts()
        );
        assert_eq!(
            sorted_rects(render_waste_leaf_rects(&render_tree)),
            sorted_rects(sheet.free_rects.clone())
        );
        assert_eq!(
            sorted_cut_piece_leaf_records(render_cut_piece_leaf_records(&render_tree)),
            placed_piece_records(&sheet.placed_pieces)
        );
    }

    #[test]
    fn guillotine_solution_sheet_carries_representable_cutting_guide() {
        let mut sheet = guillotine_sheet_with_size(100, 80, 3);
        let first_cut = cut_instance_with_id(10, 60, 50);
        let second_cut = cut_instance_with_id(11, 20, 20);

        assert!(sheet.try_insert(&first_cut, BaselineGuillotineBackend::default_heuristic()));
        assert!(sheet.try_insert(&second_cut, BaselineGuillotineBackend::default_heuristic()));

        let expected_waste = sorted_rects(sheet.free_rects.clone());
        let expected_pieces = placed_piece_records(&sheet.placed_pieces);
        let solution_sheet = sheet.into_solution_sheet();
        let guide = solution_sheet
            .cutting_guide
            .as_ref()
            .expect("representable guillotine tree should be emitted");

        assert_eq!(sorted_rects(render_waste_leaf_rects(guide)), expected_waste);
        assert_eq!(
            sorted_cut_piece_leaf_records(render_cut_piece_leaf_records(guide)),
            expected_pieces
        );
    }

    #[test]
    fn guillotine_solution_sheet_omits_invalidated_cutting_guide() {
        let mut sheet = guillotine_sheet_with_size(100, 100, 1);
        sheet.free_rects = vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 40,
            },
            Rect {
                x: 0,
                y: 41,
                width: 100,
                length: 59,
            },
        ];
        sheet.merge_free_rects_and_invalidate_slicing_tree();

        let solution_sheet = sheet.into_solution_sheet();

        assert_eq!(solution_sheet.cutting_guide, None);
    }

    #[test]
    fn nested_solution_sheet_has_no_guillotine_cutting_guide() {
        let sheet = nested_sheet_with_free_rects(vec![Rect {
            x: 0,
            y: 0,
            width: 100,
            length: 100,
        }]);

        let solution_sheet = sheet.into_solution_sheet();

        assert_eq!(solution_sheet.cutting_guide, None);
    }

    #[test]
    fn guillotine_slicing_tree_leaves_match_sheet_state_while_representable() {
        let mut sheet = guillotine_sheet_with_size(100, 80, 3);
        let first_cut = cut_instance_with_id(10, 60, 50);
        let second_cut = cut_instance_with_id(11, 20, 20);

        assert!(sheet.try_insert(&first_cut, BaselineGuillotineBackend::default_heuristic()));
        assert!(sheet.try_insert(&second_cut, BaselineGuillotineBackend::default_heuristic()));

        let tree = sheet
            .slicing_tree
            .as_ref()
            .expect("unmerged inserts should keep the slicing tree representable");

        assert_eq!(
            sorted_rects(tree.free_leaf_rects()),
            sorted_rects(sheet.free_rects.clone())
        );
        assert_eq!(
            sorted_cut_piece_leaf_records(tree.cut_piece_leaf_records()),
            placed_piece_records(&sheet.placed_pieces)
        );
    }

    #[test]
    fn guillotine_free_rect_merge_invalidates_slicing_tree() {
        let mut sheet = guillotine_sheet_with_size(100, 100, 1);
        sheet.free_rects = vec![
            Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 40,
            },
            Rect {
                x: 0,
                y: 41,
                width: 100,
                length: 59,
            },
        ];

        assert!(sheet.slicing_tree.is_some());

        sheet.merge_free_rects_and_invalidate_slicing_tree();

        assert_eq!(
            sheet.free_rects,
            vec![Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            }]
        );
        assert_eq!(sheet.slicing_tree, None);
    }

    #[test]
    fn guillotine_merge_free_rects_joins_adjacent_rects_across_kerf() {
        assert_eq!(
            merge_adjacent_rects(
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    length: 40,
                },
                Rect {
                    x: 0,
                    y: 41,
                    width: 100,
                    length: 59,
                },
                1,
            ),
            Some(Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            })
        );
        assert_eq!(
            merge_adjacent_rects(
                Rect {
                    x: 0,
                    y: 0,
                    width: 40,
                    length: 100,
                },
                Rect {
                    x: 41,
                    y: 0,
                    width: 59,
                    length: 100,
                },
                1,
            ),
            Some(Rect {
                x: 0,
                y: 0,
                width: 100,
                length: 100,
            })
        );
    }

    #[test]
    fn shuffled_candidate_seed_description_keeps_seed_and_index_inspectable() {
        let description = CandidateSeedDescription::ShuffledFirstFit { seed: 42, index: 3 };

        assert_eq!(
            description,
            CandidateSeedDescription::ShuffledFirstFit { seed: 42, index: 3 }
        );
    }

    #[test]
    fn shuffle_cut_order_is_seeded_and_reproducible() {
        let project = project(
            vec![stock_with_size(1, 200, 200, 1)],
            vec![
                cut_with_size(10, 50, 10, 1),
                cut_with_size(20, 30, 40, 1),
                cut_with_size(30, 50, 20, 1),
                cut_with_size(40, 10, 90, 1),
            ],
        );
        let instance = expand_project(&project).expect("project should expand");
        let cut_ids_for_seed = |seed| {
            shuffle_cut_order(&instance.cuts, seed)
                .iter()
                .map(|key| key.cut_id)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            cut_ids_for_seed(1),
            vec![PieceId(20), PieceId(30), PieceId(40), PieceId(10)]
        );
        assert_eq!(
            cut_ids_for_seed(2),
            vec![PieceId(30), PieceId(10), PieceId(40), PieceId(20)]
        );
        assert_eq!(cut_ids_for_seed(1), cut_ids_for_seed(1));
        assert_ne!(cut_ids_for_seed(1), cut_ids_for_seed(2));
    }

    #[test]
    fn candidate_place_cut_first_fit_tracks_unused_cut() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 60, 100, 2)],
        );
        let instance = expand_project(&project).expect("project should expand");
        let kerf_width = instance.kerf_width;
        let cut_catalog = CutCatalog::new(instance.cuts.clone());
        let mut candidate = Candidate::<BaselineGuillotineBackend>::new(
            StockInventory::new(instance.stock),
            cut_catalog,
        );
        let heuristic = BaselineGuillotineBackend::default_heuristic();

        assert!(candidate.place_cut_first_fit(&instance.cuts[0], kerf_width, heuristic));
        assert!(!candidate.place_cut_first_fit(&instance.cuts[1], kerf_width, heuristic));

        assert_eq!(candidate.bins.len(), 1);
        assert_eq!(candidate.remaining_stock_count(), 0);
        assert_eq!(
            candidate
                .unused_cuts
                .iter()
                .map(|cut| (cut.cut_id, cut.instance))
                .collect::<Vec<_>>(),
            vec![(PieceId(10), 1)]
        );
    }

    #[test]
    fn candidate_exposes_internal_stock_and_cut_keys_without_render_solution() {
        let project = project(
            vec![stock_with_size(1, 103, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        let instance = expand_project(&project).expect("project should expand");

        let candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);

        assert_eq!(
            candidate.used_stock_keys(),
            vec![StockInstanceKey {
                stock_id: PieceId(1),
                instance: 0,
            }]
        );
        assert_eq!(
            candidate.used_stock_instances(),
            vec![StockInstance {
                stock_id: PieceId(1),
                instance: 0,
                width: 103,
                length: 50,
                pattern: PatternDirection::None,
            }]
        );
        assert_eq!(
            candidate.placed_cut_keys(),
            vec![
                CutInstanceKey {
                    cut_id: PieceId(10),
                    instance: 0,
                },
                CutInstanceKey {
                    cut_id: PieceId(10),
                    instance: 1,
                },
            ]
        );
    }

    #[test]
    fn candidate_cut_catalog_resolves_placed_keys_to_original_cut_instances() {
        let project = project(
            vec![stock_with_size(1, 103, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        let instance = expand_project(&project).expect("project should expand");

        let candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);
        let placed_keys = candidate.placed_cut_keys();

        let resolved_cuts = placed_keys
            .into_iter()
            .map(|key| {
                candidate
                    .cut_for_key(key)
                    .map(|cut| (cut.cut_id, cut.instance, cut.width, cut.length))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            resolved_cuts,
            vec![
                Some((PieceId(10), 0, 50, 50)),
                Some((PieceId(10), 1, 50, 50)),
            ]
        );
    }

    #[test]
    fn candidate_builds_placed_cut_records_for_future_repair() {
        let project = project(
            vec![stock_with_size(1, 103, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        let instance = expand_project(&project).expect("project should expand");

        let candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);
        let records = candidate
            .placed_cut_records()
            .expect("all placed cuts should resolve through the catalog");

        assert_eq!(
            records
                .iter()
                .map(|record| (record.key, record.cut.cut_id, record.cut.instance))
                .collect::<Vec<_>>(),
            vec![
                (
                    CutInstanceKey {
                        cut_id: PieceId(10),
                        instance: 0,
                    },
                    PieceId(10),
                    0,
                ),
                (
                    CutInstanceKey {
                        cut_id: PieceId(10),
                        instance: 1,
                    },
                    PieceId(10),
                    1,
                ),
            ]
        );
    }

    #[test]
    fn candidate_resolves_cut_key_lists_for_future_reinsert() {
        let project = project(
            vec![stock_with_size(1, 103, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        let instance = expand_project(&project).expect("project should expand");

        let candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);
        let keys = vec![
            CutInstanceKey {
                cut_id: PieceId(10),
                instance: 1,
            },
            CutInstanceKey {
                cut_id: PieceId(10),
                instance: 0,
            },
        ];

        let cuts = candidate
            .cuts_for_keys(&keys)
            .expect("all requested keys should resolve");
        assert_eq!(
            cuts.iter()
                .map(|cut| (cut.cut_id, cut.instance, cut.width, cut.length))
                .collect::<Vec<_>>(),
            vec![(PieceId(10), 1, 50, 50), (PieceId(10), 0, 50, 50)]
        );

        assert!(candidate
            .cuts_for_keys(&[CutInstanceKey {
                cut_id: PieceId(999),
                instance: 0,
            }])
            .is_none());
    }

    #[test]
    fn candidate_removes_placed_cuts_by_rebuilding_bin_and_tracks_them_unused() {
        let mut project = project(
            vec![stock_with_size(1, 103, 50, 1)],
            vec![cut_with_size(10, 50, 50, 2)],
        );
        project.settings.kerf_width = 3;
        let instance = expand_project(&project).expect("project should expand");
        let kerf_width = instance.kerf_width;
        let heuristic = BaselineGuillotineBackend::default_heuristic();
        let mut candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);
        let removed_key = CutInstanceKey {
            cut_id: PieceId(10),
            instance: 0,
        };

        let removed = candidate
            .remove_placed_cuts_from_bin(0, &[removed_key], kerf_width, heuristic)
            .expect("removal should rebuild the bin");

        assert_eq!(
            removed
                .iter()
                .map(|record| (record.key, record.cut.width, record.cut.length))
                .collect::<Vec<_>>(),
            vec![(removed_key, 50, 50)]
        );
        assert_eq!(
            candidate.placed_cut_keys(),
            vec![CutInstanceKey {
                cut_id: PieceId(10),
                instance: 1,
            }]
        );
        assert_eq!(
            candidate
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>(),
            vec![removed_key]
        );
        assert!(!candidate.is_valid());

        assert_eq!(
            candidate.reinsert_unused_first_fit(kerf_width, heuristic),
            1
        );
        assert!(candidate.is_valid());
        assert!(candidate.unused_cuts.is_empty());
        assert_solution_within_bounds_and_non_overlapping(
            &candidate.into_solution(LayoutKind::Guillotine),
        );
    }

    #[test]
    fn candidate_reinsert_unused_first_fit_does_not_duplicate_failed_reinserts() {
        let project = project(
            vec![stock_with_size(1, 100, 100, 1)],
            vec![cut_with_size(10, 101, 100, 1)],
        );
        let instance = expand_project(&project).expect("project should expand");
        let kerf_width = instance.kerf_width;
        let heuristic = BaselineGuillotineBackend::default_heuristic();
        let mut candidate = first_fit_candidate::<BaselineGuillotineBackend>(instance);
        let unused_key = CutInstanceKey {
            cut_id: PieceId(10),
            instance: 0,
        };

        assert_eq!(
            candidate
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>(),
            vec![unused_key]
        );

        assert_eq!(
            candidate.reinsert_unused_first_fit(kerf_width, heuristic),
            0
        );
        assert_eq!(
            candidate.reinsert_unused_first_fit(kerf_width, heuristic),
            0
        );
        assert_eq!(
            candidate
                .unused_cuts
                .iter()
                .map(CutInstanceKey::from)
                .collect::<Vec<_>>(),
            vec![unused_key]
        );
    }

    #[test]
    fn baseline_allows_patternless_cut_on_patterned_stock() {
        let project = project(
            vec![stock_with_pattern(
                1,
                100,
                200,
                PatternDirection::ParallelToLength,
            )],
            vec![cut_with_pattern(
                10,
                100,
                200,
                PatternDirection::None,
                false,
            )],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("no cut pattern should not constrain patterned stock");

        assert_eq!(
            solution.sheets[0].placed_pieces[0].pattern,
            PatternDirection::None
        );
    }

    #[test]
    fn baseline_allows_patterned_cut_on_patternless_stock() {
        let project = project(
            vec![stock_with_pattern(1, 100, 200, PatternDirection::None)],
            vec![cut_with_pattern(
                10,
                100,
                200,
                PatternDirection::ParallelToWidth,
                false,
            )],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("patternless stock should not constrain patterned cuts");

        assert_eq!(
            solution.sheets[0].placed_pieces[0].pattern,
            PatternDirection::ParallelToWidth
        );
    }

    #[test]
    fn baseline_rejects_pattern_mismatch_when_both_sides_have_different_patterns() {
        let project = project(
            vec![stock_with_pattern(
                1,
                100,
                200,
                PatternDirection::ParallelToLength,
            )],
            vec![cut_with_pattern(
                10,
                100,
                200,
                PatternDirection::ParallelToWidth,
                false,
            )],
        );

        let error = BaselineOptimizer
            .optimize(&project)
            .expect_err("matching dimensions are not enough when both patterns conflict");

        assert_eq!(error, OptimizeError::NoSolution);
    }

    #[test]
    fn baseline_rotates_piece_when_pattern_and_dimensions_allow_it() {
        let project = project(
            vec![stock_with_pattern(
                1,
                50,
                100,
                PatternDirection::ParallelToLength,
            )],
            vec![cut_with_pattern(
                10,
                100,
                50,
                PatternDirection::ParallelToWidth,
                true,
            )],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("rotated pattern should match stock pattern");

        let placed = &solution.sheets[0].placed_pieces[0];
        assert!(placed.rotated);
        assert_eq!(placed.pattern, PatternDirection::ParallelToLength);
        assert_eq!(
            placed.rect,
            Rect {
                x: 0,
                y: 0,
                width: 50,
                length: 100,
            }
        );
    }

    #[test]
    fn baseline_rotates_patternless_piece_when_only_rotated_dimensions_fit() {
        let project = project(
            vec![stock_with_size(1, 50, 100, 1)],
            vec![CutPiece {
                can_rotate: true,
                ..cut_with_size(10, 100, 50, 1)
            }],
        );

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("patternless rotatable cut should fit rotated");

        let placed = &solution.sheets[0].placed_pieces[0];
        assert!(placed.rotated);
        assert_eq!(placed.pattern, PatternDirection::None);
        assert_eq!(placed.rect.width, 50);
        assert_eq!(placed.rect.length, 100);
    }

    #[test]
    fn baseline_solution_places_pieces_within_bounds_without_overlap() {
        let mut project = project(
            vec![stock_with_size(1, 206, 206, 1)],
            vec![cut_with_size(10, 100, 100, 4)],
        );
        project.settings.kerf_width = 3;

        let solution = BaselineOptimizer
            .optimize(&project)
            .expect("four pieces plus kerfs should fit in a simple grid");

        assert_eq!(solution.sheets.len(), 1);
        assert_eq!(solution.sheets[0].placed_pieces.len(), 4);
        assert_solution_within_bounds_and_non_overlapping(&solution);
    }
}
