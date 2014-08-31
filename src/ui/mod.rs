// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

pub mod piston;

pub trait UI : Drop {
	fn run(&mut self);
}
