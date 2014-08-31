// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

use hex2d;
use std::rc::Rc;
use std::cell::RefCell;
use creature::{Creature};


#[deriving(Eq)]
#[deriving(PartialEq)]
#[deriving(Clone)]
pub enum TileType {
    Floor,
    GlassWall,
    Wall,
    Sand,
}

#[deriving(Clone)]
pub struct Tile<'a> {
    pub tiletype : TileType,
    pub creature : Option<Rc<RefCell<Creature<'a>>>>,
}

impl<'a> Tile<'a> {
    pub fn opaqueness(&self) -> uint {
        let o = match self.tiletype {
            Wall => 1000000,
            GlassWall => 3,
            _ => 1
        };
        o + if self.creature.is_some() { 4 } else { 0 }
    }

    pub fn is_passable_type(&self) -> bool {
        match self.tiletype {
            Wall|GlassWall => false,
            _ => true
        }
    }

    pub fn is_passable(&self) -> bool {
        self.is_passable_type() && self.creature.is_none()
    }

}

impl TileType {
    pub fn move_delay(&self) -> uint {
        match self {
            &Sand => 6,
            _ => 0
        }
    }
}

pub type Map<'a> = hex2d::Map<Tile<'a>>;
