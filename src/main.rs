// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

#![feature(globs)]
#![feature(phase)]

extern crate piston;

extern crate cgmath;
extern crate device;
extern crate glfw;
extern crate gl;
extern crate glfw_game_window;
extern crate gfx;
#[phase(plugin)]
extern crate gfx_macros;
extern crate time;
extern crate native;
extern crate hex2d;
#[phase(plugin, link)] extern crate log;
extern crate input;

mod ui;
mod game;
mod creature;
mod ai;
mod map;

#[start]
fn start(argc: int, argv: *const *const u8) -> int {
    native::start(argc, argv, main)
}

#[main]
pub fn main() {
    let (mut ui, mut window) = ui::piston::PistonUI::new();

    let mut game = game::GameState::new();

    game.randomize_map();

    ui.run(&mut window, &mut game);
}
