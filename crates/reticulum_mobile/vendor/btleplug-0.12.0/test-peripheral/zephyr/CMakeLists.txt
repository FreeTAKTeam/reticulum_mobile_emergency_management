# SPDX-License-Identifier: Apache-2.0

cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(btleplug_test_peripheral)

target_sources(app PRIVATE
    src/main.c
    src/gatt_profile.c
    src/control_service.c
    src/test_handlers.c
)

target_include_directories(app PRIVATE src)
