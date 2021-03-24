# Install script for directory: /home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Release")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Install shared libraries without execute permission?
if(NOT DEFINED CMAKE_INSTALL_SO_NO_EXE)
  set(CMAKE_INSTALL_SO_NO_EXE "0")
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "FALSE")
endif()

# Set default install directory permissions.
if(NOT DEFINED CMAKE_OBJDUMP)
  set(CMAKE_OBJDUMP "/usr/bin/objdump")
endif()

if("x${CMAKE_INSTALL_COMPONENT}x" STREQUAL "xUnspecifiedx" OR NOT CMAKE_INSTALL_COMPONENT)
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/lib/libcfltk.a")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
file(INSTALL DESTINATION "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/lib" TYPE STATIC_LIBRARY FILES "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/libcfltk.a")
endif()

if("x${CMAKE_INSTALL_COMPONENT}x" STREQUAL "xUnspecifiedx" OR NOT CMAKE_INSTALL_COMPONENT)
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_box.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_browser.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_button.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_dialog.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_draw.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_enums.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_group.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_image.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_input.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_menu.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_misc.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_output.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_printer.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_surface.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_table.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_text.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_tree.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_valuator.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_widget.h;/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk/cfl_window.h")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
file(INSTALL DESTINATION "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/include/cfltk" TYPE FILE FILES
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_box.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_browser.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_button.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_dialog.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_draw.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_enums.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_group.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_image.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_input.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_menu.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_misc.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_output.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_printer.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_surface.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_table.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_text.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_tree.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_valuator.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_widget.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/include/cfl_window.h"
    )
endif()

if("x${CMAKE_INSTALL_COMPONENT}x" STREQUAL "xUnspecifiedx" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig.cmake")
    file(DIFFERENT EXPORT_FILE_CHANGED FILES
         "$ENV{DESTDIR}/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig.cmake"
         "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/CMakeFiles/Export/_home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig.cmake")
    if(EXPORT_FILE_CHANGED)
      file(GLOB OLD_CONFIG_FILES "$ENV{DESTDIR}/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig-*.cmake")
      if(OLD_CONFIG_FILES)
        message(STATUS "Old export file \"$ENV{DESTDIR}/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig.cmake\" will be replaced.  Removing files [${OLD_CONFIG_FILES}].")
        file(REMOVE ${OLD_CONFIG_FILES})
      endif()
    endif()
  endif()
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig.cmake")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
file(INSTALL DESTINATION "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk" TYPE FILE FILES "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/CMakeFiles/Export/_home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig.cmake")
  if("${CMAKE_INSTALL_CONFIG_NAME}" MATCHES "^([Rr][Ee][Ll][Ee][Aa][Ss][Ee])$")
    list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
     "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig-release.cmake")
    if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
        message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
    endif()
    if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
        message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
    endif()
file(INSTALL DESTINATION "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk" TYPE FILE FILES "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/CMakeFiles/Export/_home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfig-release.cmake")
  endif()
endif()

if("x${CMAKE_INSTALL_COMPONENT}x" STREQUAL "xUnspecifiedx" OR NOT CMAKE_INSTALL_COMPONENT)
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk/cfltkConfigVersion.cmake")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
file(INSTALL DESTINATION "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/share/cmake/cfltk" TYPE FILE FILES "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/cfltkConfigVersion.cmake")
endif()

if(NOT CMAKE_INSTALL_LOCAL_ONLY)
  # Include the install script for each subdirectory.
  include("/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/fltk/cmake_install.cmake")

endif()

if(CMAKE_INSTALL_COMPONENT)
  set(CMAKE_INSTALL_MANIFEST "install_manifest_${CMAKE_INSTALL_COMPONENT}.txt")
else()
  set(CMAKE_INSTALL_MANIFEST "install_manifest.txt")
endif()

string(REPLACE ";" "\n" CMAKE_INSTALL_MANIFEST_CONTENT
       "${CMAKE_INSTALL_MANIFEST_FILES}")
file(WRITE "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/${CMAKE_INSTALL_MANIFEST}"
     "${CMAKE_INSTALL_MANIFEST_CONTENT}")
