mod models;
use {
    cascade::cascade,
    comfy_table::{modifiers, presets, Table},
    fltk::{
        app,
        button::Button,
        dialog::{alert_default, message_default},
        draw,
        enums::{CallbackTrigger, Color, Cursor, Event, Font},
        frame::Frame,
        group::{Flex, FlexType, Wizard},
        image::SvgImage,
        input::{Input, InputType},
        menu::{Choice, MenuButton, MenuButtonType},
        prelude::*,
        text::{TextBuffer, TextDisplay},
        window::Window,
    },
    std::{cell::RefCell, rc::Rc},
};

pub const NAME: &str = "FreeCut";
const PAD: i32 = 10;
const HEIGHT: i32 = PAD * 3;
const WIDTH: i32 = HEIGHT * 3;

fn main() -> Result<(), FltkError> {
    let app = app::App::default();
    cascade!(
        Window::default().with_size(360, 640).center_screen();
        ..set_label(NAME);
        ..set_xclass("freecut");
        ..size_range(360, 640, 0, 0);
        ..set_icon(Some(
            SvgImage::from_data(include_str!("../assets/logo.svg")).unwrap(),
        ));
        ..set_callback(move |window| {
            if app::event() == Event::Close {
                window.child(0).unwrap().do_callback();
                app::quit();
            }
        });
        ..make_resizable(true);
        ..add(&cascade!(
            Wizard::default_fill();
            ..set_callback(move |wizard| wizard.child(0).unwrap().do_callback());
            ..add(&page_optimizer());
            ..add(&page_settings());
            ..add(&page_doc("Manual", include_str!("../README.md")));
            ..add(&page_doc("License", include_str!("../LICENSE")));
            ..end();
            ..handle(add_menu);
        ));
        ..end();
    )
    .show();
    app::set_font(Font::CourierBold);
    app.run()
}

enum Message {
    Update = 41,
}

impl Message {
    const fn event(self) -> Event {
        Event::from_i32(self as i32)
    }
}

fn page_optimizer() -> Flex {
    const PATTERNS: [&str; 3] = ["none", "width", "length"];
    const KINDS: [&str; 2] = ["cutpiece", "stockpiece"];
    const UNITS: [&str; 3] = ["mm", "inch", "foot"];
    const ERROR_RANGE: &str = "Value is out of range!";
    const ERROR_PIECE: &str = "Add at least one stockpiece to the draft list!\nAdd at least one cutpiece to the draft list!\n";
    let state = Rc::new(RefCell::new(models::Model::default()));
    const UPDATE: Event = Message::Update.event();
    cascade!(
        Flex::default_fill().with_label("Optimizer").column();
        ..set_margin(PAD);
        ..set_pad(0);
        ..set_callback({
            let state = Rc::clone(&state);
            move |_| state.borrow().save()
        });
        ..add(&cascade!(
            Flex::default_fill();
            ..set_margin(0);
            ..set_pad(PAD);
            ..fixed(&Frame::default(), WIDTH * 2);
            ..add(&cascade!(
                Flex::default_fill().column();
                ..set_margin(0);
                ..set_pad(PAD);
                ..fixed(&cascade!(
                    Button::default().with_label("@#refresh");
                    ..set_label_color(Color::Red);
                    ..set_tooltip("CLEAN");
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |_| {
                            state.borrow_mut().clean();
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                ), HEIGHT);
                ..fixed(&cascade!(
                    Choice::default().with_label("piece type");
                    ..add_choice(&KINDS.join("|"));
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().piece_kind(choice.value());
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    ..handle({
                        let state = Rc::clone(&state);
                        move |choice, event| {
                            if event == UPDATE {
                                choice.set_value(state.borrow().piece().kind());
                            }
                            false
                        }
                    });
                    ..handle_event(UPDATE);
                ), HEIGHT);
                ..fixed(&cascade!(
                    Input::default().with_label("width:").with_type(InputType::Float);
                    ..set_trigger(CallbackTrigger::Changed);
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if state.borrow().allowed_range(&value) {
                                    state.borrow_mut().piece_width(value);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    ..handle({
                        let state = Rc::clone(&state);
                        move |input, event| {
                            if event == UPDATE {
                                input.set_value(&state.borrow().piece().width());
                            }
                            false
                        }
                    });
                    ..handle_event(UPDATE);
                ), HEIGHT);
                ..fixed(&cascade!(
                    Input::default().with_label("length:").with_type(InputType::Float);
                    ..set_trigger(CallbackTrigger::Changed);
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if state.borrow().allowed_range(&value) {
                                    state.borrow_mut().piece_length(value);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    ..handle({
                        let state = Rc::clone(&state);
                        move |input, event| {
                            if event == UPDATE {
                                input.set_value(&state.borrow().piece().length());
                            }
                            false
                        }
                    });
                    ..handle_event(UPDATE);
                ), HEIGHT);
                ..fixed(&cascade!(
                    Input::default().with_label("amount:").with_type(InputType::Int);
                    ..set_trigger(CallbackTrigger::Changed);
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if state.borrow().allowed_range(&value) {
                                    state.borrow_mut().piece_amount(value as usize);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    ..handle({
                        let state = Rc::clone(&state);
                        move |input, event| {
                            if event == UPDATE {
                                input.set_value(&state.borrow().piece().amount());
                            }
                            false
                        }
                    });
                    ..handle_event(UPDATE);
                ), HEIGHT);
                ..fixed(&cascade!(
                    Choice::default().with_label("pattern (parallel to):");
                    ..add_choice(&PATTERNS.join("|"));
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().piece_pattern(choice.value());
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                    ..handle({
                        let state = Rc::clone(&state);
                        move |choice, event| {
                            if event == UPDATE {
                                choice.set_value(state.borrow().piece().pattern());
                            }
                            false
                        }
                    });
                    ..handle_event(UPDATE);
                ), HEIGHT);
                ..fixed(&cascade!(
                    Button::default().with_label("@#+");
                    ..set_label_color(Color::Green);
                    ..set_tooltip("ADD PIECE");
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |_| {
                            state.borrow_mut().add();
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                ), HEIGHT);
                ..fixed(&cascade!(
                    Choice::default().with_label("unit");
                    ..add_choice(&UNITS.join("|"));
                    ..set_value(state.borrow().unit());
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().set_unit(choice.value());
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                ), HEIGHT);
                ..fixed(&cascade!(
                    Input::default().with_label("cut_width:").with_type(InputType::Float);
                    ..set_value(&state.borrow().width());
                    ..set_trigger(CallbackTrigger::Changed);
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |input| {
                            if let Ok(value) = input.value().parse::<f32>() {
                                if (0.0..15.0).contains(&value) {
                                    state.borrow_mut().set_width(value);
                                    input.set_color(Color::Background2);
                                } else {
                                    input.set_color(Color::Red);
                                    alert_default(ERROR_RANGE);
                                }
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                ), HEIGHT);
                ..fixed(&cascade!(
                    Choice::default().with_label("layout:");
                    ..add_choice("guillotine|nested");
                    ..set_value(state.borrow().layout());
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |choice| {
                            state.borrow_mut().set_layout(choice.value());
                        }
                    });
                ), HEIGHT);
                ..fixed(&cascade!(
                    Button::default().with_label("@#circle");
                    ..set_tooltip("OPTIMIZE");
                    ..set_callback({
                        let state = Rc::clone(&state);
                        move |_| {
                            let list: Vec<i32> = state.borrow().pieces().iter().map(|x| x.kind()).collect();
                            if list.len() > 1 && list.contains(&0) && list.contains(&1) {
                                message_default(&state.borrow_mut().optimize());
                            } else {
                                alert_default(ERROR_PIECE);
                            }
                            app::handle_main(UPDATE).unwrap();
                        }
                    });
                ), HEIGHT);
                ..add(&Frame::default());
                ..end();
            ));
        ));
        ..add(&Frame::default());
        ..add(&cascade!(
            TextDisplay::default();
            ..set_tooltip("Output");
            ..set_buffer(TextBuffer::default());
            ..handle({
                let state = Rc::clone(&state);
                move |display, event| {
                    if event == UPDATE {
                        display.buffer().unwrap().set_text({
                            let state = state.borrow();
                            let unit = UNITS[state.unit() as usize];
                            let mut table = Table::new();
                            table.load_preset(presets::UTF8_FULL);
                            table.apply_modifier(modifiers::UTF8_ROUND_CORNERS);
                            table.set_header(["TYPE", &format!("WIDTH ({unit})"), &format!("LENGTH ({unit})"), "AMOUNT", "PATTERN"]);
                            for piece in state.pieces() {
                                table.add_row([
                                    KINDS[piece.kind() as usize],
                                    &piece.width(),
                                    &piece.length(),
                                    &piece.amount(),
                                    PATTERNS[piece.pattern() as usize],
                                ]);
                            }
                            &table.to_string()
                        });
                    }
                    false
                }
            });
            ..handle_event(UPDATE);
        ));
        ..end();
        ..handle(add_orientation);
        ..handle_event(Event::Resize);
    )
}

fn page_settings() -> Flex {
    cascade!(
        Flex::default_fill().with_label("Settings");
        ..set_margin(PAD);
        ..set_pad(PAD);
        ..add(&Frame::default());
        ..fixed(&cascade!(
            Flex::default_fill().column();
            ..set_pad(PAD);
            ..set_margin(PAD);
            ..add(&Frame::default());
            ..add(&cascade!(
                Flex::default_fill();
                ..fixed(&Frame::default(), WIDTH);
                ..add(&cascade!(
                    Flex::default_fill().column();
                    ..set_color(Color::Foreground);
                    ..set_pad(PAD);
                    ..fixed(&cascade!(
                        Choice::default().with_label("Theme");
                        ..add_choice("Solarized Light|Solarized Dark");
                        ..set_value(0);
                        ..set_callback(move |choice| {
                            let color = [
                                [ //LIGHT
                                    0xeee8d5, //base2
                                    0xfdf6e3, //base3
                                    0x586e75, //base01
                                    0xcb4b16, //orange
                                    0xb58900, //yellow
                                ],
                                [ //DARK
                                    0x073642, //base02
                                    0x002b36, //base03
                                    0x93a1a1, //base1
                                    0x6c71c4, //violet
                                    0x268bd2, //blue
                                ],
                            ][choice.value() as usize];
                            app::set_scheme(match choice.value() {
                                0 => app::Scheme::Oxy,
                                _ => app::Scheme::Gtk,
                            });
                            let (r, g, b) = Color::from_hex(color[0]).to_rgb();
                            app::set_background_color(r, g, b);
                            let (r, g, b) = Color::from_hex(color[1]).to_rgb();
                            app::set_background2_color(r, g, b);
                            let (r, g, b) = Color::from_hex(color[2]).to_rgb();
                            app::set_foreground_color(r, g, b);
                            let (r, g, b) = Color::from_hex(color[3]).to_rgb();
                            app::set_selection_color(r, g, b);
                            let (r, g, b) = Color::from_hex(color[4]).to_rgb();
                            app::set_inactive_color(r, g, b);
                            for (color, hex) in [
                                (Color::Yellow, 0xb58900),
                                (Color::Red, 0xdc322f),
                                (Color::Magenta, 0xd33682),
                                (Color::Blue, 0x268bd2),
                                (Color::Cyan, 0x2aa198),
                                (Color::Green, 0x859900),
                            ] {
                                let (r, g, b) = Color::from_hex(hex).to_rgb();
                                app::set_color(color, r, g, b);
                            }
                            app::set_visible_focus(false);
                            app::redraw();
                        });
                        ..do_callback();
                    ), HEIGHT);
                    ..end();
                ));
                ..end();
            ));
            ..add(&Frame::default());
            ..end();
        ), WIDTH * 3);
        ..add(&Frame::default());
        ..end();
    )
}

fn page_doc(title: &str, body: &str) -> Flex {
    cascade!(
        Flex::default_fill().with_label(title);
        ..set_margin(PAD);
        ..add(&cascade!(
            TextDisplay::default();
            ..set_buffer(TextBuffer::default());
            ..insert(body);
        ));
        ..end();
    )
}

fn add_orientation(flex: &mut Flex, event: Event) -> bool {
    if event == Event::Resize {
        if let Some(window) = flex.window() {
            flex.set_type(match window.w() < window.h() {
                true => FlexType::Column,
                false => FlexType::Row,
            });
            flex.fixed(&flex.child(0).unwrap(), 11 * HEIGHT + 10 * PAD);
            flex.fixed(&flex.child(1).unwrap(), PAD);
        }
        return true;
    }
    false
}

fn add_menu(wizard: &mut Wizard, event: Event) -> bool {
    match event {
        Event::Push => match app::event_mouse_button() {
            app::MouseButton::Right => {
                cascade!(
                    MenuButton::default();
                    ..add_choice(
                        &(0..wizard.children()).map(|x| {
                            let label = wizard.child(x).unwrap().label();
                            if wizard.try_current_widget().unwrap().label() == label {
                                format!("@->  {}", label)
                            } else {
                                format!("@-  {}", label)
                            }
                        })
                        .collect::<Vec<String>>()
                        .join("|")
                    );
                    ..set_type(MenuButtonType::Popup3);
                    ..set_callback({
                        let mut wizard = wizard.clone();
                        move |menu| {
                            wizard.try_current_widget().unwrap().do_callback();
                            wizard.set_current_widget(&wizard.child(menu.value()).unwrap());
                        }
                    });
                )
                .popup();
                true
            }
            _ => false,
        },
        Event::Enter => {
            draw::set_cursor(Cursor::Hand);
            true
        }
        Event::Leave => {
            draw::set_cursor(Cursor::Arrow);
            true
        }
        _ => false,
    }
}
