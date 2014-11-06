// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

use creature::{Creature};
use creature::{Race,Human,Scout,Grunt,Heavy};
use hex2d;
use hex2d::{Point,Position,Direction};
use hex2d::{Forward,Backward};
use map::{Tile,Map};
use map::{Wall,Floor,GlassWall,Sand};
use std::rand;
use std::rand::Rng;
use std::cell::{RefCell};
use std::rc::{Rc};
use std::vec::Vec;
use std::slice::Items;

pub type CreatureRef = Rc<RefCell<Creature>>;
pub type Creatures = Vec<CreatureRef>;

pub struct GameState {
    pub map : Box<Map>,
    pub player : Option<CreatureRef>,
    rng : rand::TaskRng,
    creatures: Creatures,
    tick : uint,
}

#[deriving(Show)]
pub enum Action {
    Run(Direction),
    Move(Direction),
    Turn(Direction),
    Melee(Direction),
    Use,
    Wait
}

impl GameState {
    pub fn new() -> GameState {
        let map = box hex2d::Map::new(100, 100, Tile {
            tiletype: Floor,
            creature: None,
        }
        );
        GameState {
            player: None,
            rng: rand::task_rng(),
            map: map,
            creatures: Vec::new(),
            tick: 0,
        }
    }

    fn spawn(&mut self, cr : Creature) -> Option<Rc<RefCell<Creature>>>  {
        if !self.map.at(*cr.p()).is_passable() {
            None
        } else {
            let p = *cr.p();
            let pl = Rc::new(RefCell::new(cr));
            self.map.mut_at(p).creature = Some(pl.clone());
            self.creatures.push(pl.clone());
            Some(pl.clone())
        }
    }


    fn spawn_random(&mut self, player : bool, race : Race) -> Rc<RefCell<Creature>> {
        loop {
            let pos = self.map.wrap(self.rng.gen::<Position>());
            let cr = Creature::new(&*self.map, pos, player, race);
            match self.spawn(cr) {
                Some(cr) => return cr,
                None => {}
            }
        }
    }

    fn move_creature_if_possible(&mut self, cr : &mut Creature, pos : Position) {
        let cr_p = *cr.p();
        let pos_p = pos.p;
        if pos_p == cr_p {
            cr.pos_set(&*self.map, pos);
            return;
        }
        if !self.map.at(pos_p).is_passable() {
            return;
        }

        match self.map.at(pos_p).creature {
            Some(_) => { },
            None => {
                self.map.mut_at(pos_p).creature = self.map.at(cr_p).creature.clone();
                self.map.mut_at(cr_p).creature = None;
                cr.pos_set(&*self.map, pos);
            }
        }
    }

    pub fn creatures_iter(&self) -> Items<CreatureRef> {
        self.creatures.iter()
    }

    pub fn tick(&mut self) {
        let mut creatures = self.creatures.clone();

        for creature in creatures.iter_mut() {

            match creature.borrow_mut()  {
                mut cr => {
                    if cr.is_alive() {
                        if cr.needs_action() {
                            assert!(!cr.is_player());
                            cr.update_los(&*self.map);
                            cr.update_action(&*self.map);
                        }
                        let action = cr.tick();
                        match action {
                            Some(action) => {
                                self.perform_action(&mut *cr, action);
                                cr.action_done();
                            },
                            None => {}
                        }
                    }
                },
            };
        }

        self.tick += 1;
    }

    pub fn perform_action(&mut self, cr : &mut Creature, action : Action) {
        let old_pos = *cr.pos();
        cr.pos_prev_set(&*self.map, old_pos);

        match action {
            Turn(Forward)|Turn(Backward) => panic!("Illegal move"),
            Move(dir)|Run(dir) => {
                let pos = Position{ p: self.map.wrap(cr.p() + (cr.pos().dir + dir)), dir: cr.pos().dir };
                self.move_creature_if_possible(cr, pos)
            },
            Turn(dir) => {
                let pos = self.map.wrap(cr.pos() + dir);
                self.move_creature_if_possible(cr, pos)
            },
            Melee(dir) => {
                let target_p = self.map.wrap(cr.p() + (cr.pos().dir + dir));
                let target = self.map.mut_at(target_p).creature.as_ref().
                    map(|cr| cr.clone());
                if target.is_some() {
                    let target = target.unwrap();
                    let target = &mut *target.borrow_mut();
                    target.attacked_by(cr);
                    cr.attacked(target);

                    if !target.is_alive() {
                        self.map.mut_at(target_p).creature = None;
                    }
                }
            },
            _ => { }
        }
    }

    pub fn randomize_map(&mut self) {
        let height = self.map.height() as int;
        let width = self.map.width() as int;
        let area = width * height;

        for _ in range(0, area / 12) {
            let p = self.rng.gen::<Point>();
            let p = self.map.wrap(p);

            let t = match self.rng.gen_range(0u, 6) {
                0 => GlassWall,
                1 => Sand,
                _ => Wall
            };

            self.map.mut_at(p).tiletype = t;
            for &dir in hex2d::all_directions.iter() {
                let p = self.map.wrap(p + dir);
                self.map.mut_at(p).tiletype = t;
            }
        }

        for x in range(0i, width) {
            let p = Point::new(x, 0);
            self.map.mut_at(p).tiletype = Wall;
            let p = Point::new(x, height - 1);
            self.map.mut_at(p).tiletype = Wall;
        }

        for y in range(0i, height) {
            let p = Point::new(0, y);
            self.map.mut_at(p).tiletype = Wall;
            let p = Point::new(width - 1, y);
            self.map.mut_at(p).tiletype = Wall;
        }


        for _ in range(0, area / 200) {
            self.spawn_random(false, Scout);
        }

        for _ in range(0, area / 400) {
            self.spawn_random(false, Grunt);
        }

        for _ in range(0, area / 800) {
            self.spawn_random(false, Heavy);
        }

        let p = self.spawn_random(true, Human);

        self.player = Some(p);
    }

    pub fn update_player_los(&self) {
        match self.player {
            Some(ref pl) => pl.borrow_mut().update_los(&*self.map),
            None => {}
        }
    }
}
