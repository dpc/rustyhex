[![Build Status](https://travis-ci.org/dpc/rustyhex.svg?branch=master)](https://travis-ci.org/dpc/rustyhex)

# RustyHex

## Introduction
Simple roguelike written in [Rust][rust-home].

It's intendent to exercise my [Rust][rust-home] knowledge and let me play with
certain mechanisms that I'd like to see in roguelike game:

* hexagonal map
* tactical positioning (strafing, face-direction)
* tick system with actions having delay and duration

[rust-home]: http://rust-lang.org

## Overview

![RustyHex screenshot][ss04]

[ss04]: http://i.imgur.com/8xJiJxq.png

## Building

See `.travis.yml`

## Status and goals

[![Build Status](https://travis-ci.org/dpc/rustyhex.svg?branch=master)](https://travis-ci.org/dpc/rustyhex)

[Report problems and ideas][issues]

[issues]: https://github.com/dpc/rustyhex/issues

# How to play

## Basics

* Use `hjkl` or arrow keys to move.
* Press `r` to toggle between running and walking.
* Hold `Shift` to strafe (with Left/Right move)
* Hold `Ctrl` to attack (with a move)

## Mechanics

Game time is measured in tick. All creatures (including player) can issue
actions. Every action has pre and post delay. Pre-delay is a number of ticks
between issuing an and when it actually happens. Post-delay is a time after
action was performed and when next action can be issued.

### Running

Running is faster if the preceding action was also Running. This reflects some time
that it takes to get to full speed.

### Melee attack

Melee attack action has generally small pre-delay, but long post-delay.

### Wait "rubber"

Any action performed after Wait action is going to have it's pre-delay reduced.
This reflects the preparation time that allows for faster attack.
