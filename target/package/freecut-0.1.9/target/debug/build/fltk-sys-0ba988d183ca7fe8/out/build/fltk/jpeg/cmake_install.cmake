# Install script for directory: /home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/fltk/jpeg

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
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/lib" TYPE STATIC_LIBRARY FILES "/home/geri/freecut/target/package/freecut-0.1.9/target/debug/build/fltk-sys-0ba988d183ca7fe8/out/build/fltk/lib/libfltk_jpeg.a")
endif()

if("x${CMAKE_INSTALL_COMPONENT}x" STREQUAL "xUnspecifiedx" OR NOT CMAKE_INSTALL_COMPONENT)
  file(INSTALL DESTINATION "${CMAKE_INSTALL_PREFIX}/include/FL/images" TYPE FILE FILES
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/fltk/jpeg/jconfig.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/fltk/jpeg/jerror.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/fltk/jpeg/jmorecfg.h"
    "/home/geri/.cargo/registry/src/github.com-1ecc6299db9ec823/fltk-sys-0.15.14/cfltk/fltk/jpeg/jpeglib.h"
    )
endif()

