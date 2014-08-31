// Copyright 2014 Dawid Ciężarkiewicz
// See LICENSE file for more information

use creature::Scout;
use game::Action;
use game::{Turn,Move,Melee,Run,Wait};
use hex2d::Direction;
use hex2d::{Left,Right,Forward};
use hex2d::Point;
use map;
use std::rand;
use std::rand::Rng;
use std::f32::consts::PI;

use creature::CreatureState;

pub trait Actor {
    fn get_action(&mut self, map : &map::Map, _ : &CreatureState) -> Action;
    fn proceed_visible(&mut self, map : &map::Map, p : Point);
}

pub struct AIActor {
    next_turn : Direction,
    next_turn_times: int,
    last_player : Option<Point>,
}

impl AIActor {
    pub fn new() -> AIActor {
        AIActor{
            next_turn: Forward,
            next_turn_times: 0,
            last_player: None,
        }
    }

    fn chase(&mut self, map : &map::Map, cr : &CreatureState, p : Point) -> Action {
        let rel = cr.pos.relative_wrapped(map, p);

        let atan2 = (rel.y as f32).atan2(rel.x as f32);

        let marg = 0.01;
        if (atan2 > PI / 2.0) || (atan2 <= (-PI * 3.0 / 4.0 - marg)) {
            Turn(Left)
        } else if atan2 >= -1.10714871779 + marg {
            Turn(Right)
        } else {
            self.next_turn = if atan2 > -PI / 2.0 {
                Right
            } else {
                Left
            };
            self.next_turn_times = 2;

            if map.at(map.wrap(cr.pos.p + cr.pos.dir)).is_passable() {
                if cr.race == Scout {
                    Run(Forward)
                } else {
                    Move(Forward)
                }
            } else {
                Turn(self.next_turn)
            }
        }
    }

    fn roam_around(&mut self, map : &map::Map, cr : &CreatureState) -> Action {
        if self.next_turn_times > 0 {
            self.next_turn_times = self.next_turn_times - 1;
            Turn(self.next_turn)
        } else {
            let mut rng = rand::task_rng();
            loop {
                let dir = match rng.gen_range(0u, 6) {
                    0|1 => Forward,
                    2 => Left,
                    3 => Right,
                    _ => return Wait
                };

                if !map.at(map.wrap(cr.pos.p + cr.pos.dir)).is_passable() {
                    return Turn(Right)
                } else if map.at(map.wrap(cr.pos.p + (cr.pos.dir + dir))).is_passable()
                    && map.at(map.wrap(cr.pos.p + (cr.pos.dir + dir) + (cr.pos.dir + dir))).is_passable()
                        && !rng.gen_weighted_bool(8) {
                            return Move(Forward)
                        } else {
                            match dir {
                                Left|Right => return Turn(dir),
                                _ => return Turn(Right)
                            }
                        }
            }
        }
    }
}

impl Actor for AIActor {
    fn get_action(&mut self, map : &map::Map, me : &CreatureState) -> Action {
        if self.last_player.is_some() {
            if me.pos.p == self.last_player.unwrap() {
                self.last_player = None;
            } else if map.at(self.last_player.unwrap()).creature.as_ref()
                .and_then(|cr| cr.try_borrow())
                    .map(|cr| !cr.is_player())
                    .unwrap_or(false) {
                        self.last_player = None;
                    }
        }

        if self.last_player.is_some() {
            for dir in [Left,Forward,Right].iter() {
                let p = map.wrap(me.pos.p + (me.pos.dir + *dir));
                if map.at(p).creature.as_ref()
                    .and_then(|cr| cr.try_borrow())
                        .map(|cr| cr.is_player())
                        .unwrap_or(false) {
                            return Melee(*dir);
                        }
            }
        }

        if self.last_player.is_some() {
            let last_player = self.last_player.unwrap();
            self.chase(map, me, last_player)
        } else {
            self.roam_around(map, me)
        }
    }

    fn proceed_visible(&mut self, map : &map::Map, p : Point) {
        match map.at(p).creature.as_ref()
            .and_then(|cr| cr.try_borrow())
            .map(|cr| cr.is_player()) {
                Some(true) => self.last_player = Some(p),
                _=> {}
            }
    }
}
