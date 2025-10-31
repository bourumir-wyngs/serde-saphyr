// A tree-planting robot simulation demonstrating how to describe polymorphic commands
// using externally tagged enum variants with tuple/newtype payloads instead of struct
// payloads. The robot starts at the center (4,4) of a 10x8 grid (x: 0..=9, y: 0..=7).
// It executes a YAML-described program and prints the resulting field using characters:
//   '.' = empty, 'O' = oak, 'A' = acer, 'B' = birch, and 'R' = robot's final position.
// The final shape is:
//
// BB.....BB.
// ..A...A...
// ...O.O....
// ....R.....
// ...O.O....
// ..A...A...
// BB.....BB.
// ..........
//
// This example mirrors `polymorphism_tree_planting_robot.rs` but uses tuple variants,
// so YAML becomes simpler, e.g. `- go: 3`, `- turn: Left`, `- plant: Birch`.

use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
enum Tree {
    Oak,
    Acer,
    Birch,
}

impl Default for Tree {
    fn default() -> Self {
        Tree::Oak
    }
}

#[derive(Clone, Copy, Deserialize)]
enum Direction {
    Left,
    Right,
}

#[derive(Clone, Copy, Deserialize)]
enum Command {
    #[serde(rename = "go")]
    Go(usize),
    #[serde(rename = "turn")]
    Turn(Direction),
    // Allow either `plant:` (null/empty) or `plant: Birch`.
    #[serde(rename = "plant")]
    Plant(#[serde(default)] Option<Tree>),
}

type Program = Vec<Command>;

#[derive(Deserialize)]
struct Config {
    program: Program,
}

#[derive(Clone, Copy)]
enum Facing {
    North,
    East,
    South,
    West,
}

struct Robot {
    x: i32,
    y: i32,
    facing: Facing,
}

impl Robot {
    fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            facing: Facing::North,
        }
    }

    fn turn(&mut self, dir: Direction) {
        self.facing = match (self.facing, dir) {
            (Facing::North, Direction::Left) => Facing::West,
            (Facing::North, Direction::Right) => Facing::East,
            (Facing::East, Direction::Left) => Facing::North,
            (Facing::East, Direction::Right) => Facing::South,
            (Facing::South, Direction::Left) => Facing::East,
            (Facing::South, Direction::Right) => Facing::West,
            (Facing::West, Direction::Left) => Facing::South,
            (Facing::West, Direction::Right) => Facing::North,
        };
    }

    fn go(&mut self, distance: usize) {
        for _ in 0..distance {
            let (dx, dy) = match self.facing {
                Facing::North => (0, 1),
                Facing::East => (1, 0),
                Facing::South => (0, -1),
                Facing::West => (-1, 0),
            };
            let nx = self.x + dx;
            let ny = self.y + dy;
            if nx >= 0 && nx <= 9 && ny >= 0 && ny <= 7 {
                self.x = nx;
                self.y = ny;
            } else {
                // Stop at boundary; ignore remaining steps
                break;
            }
        }
    }

    fn plant(&self, field: &mut [[char; 10]; 8], tree: Tree) {
        let ch = match tree {
            Tree::Oak => 'O',
            Tree::Acer => 'A',
            Tree::Birch => 'B',
        };
        if self.x >= 0 && self.x <= 9 && self.y >= 0 && self.y <= 7 {
            field[self.y as usize][self.x as usize] = ch;
        }
    }
}

fn print_field(field: &[[char; 10]; 8], rx: i32, ry: i32) {
    for y in (0..8).rev() {
        for x in 0..10 {
            if x as i32 == rx && y as i32 == ry {
                print!("R");
            } else {
                print!("{}", field[y][x]);
            }
        }
        println!();
    }
}

fn main() {
    // Initialize empty field with dots
    let mut field = [['.'; 10]; 8];
    let mut robot = Robot::new(4, 4);

    // YAML program using tuple/newtype enum variants and anchors.
    let yaml = r#"
program:
  - go: 3
  - turn: Left
  - go: 4
  - plant: Birch
  - turn: Right # JSON-like inline also works as a bare value here
  - turn: Right
  # Define a command 'step' as an anchor at the same level as the node it refers to.
  - &step
    go: 1
  - plant: Birch
  - go: 6
  - plant: Birch
  - *step
  - plant: Birch
  - turn: Right
  - *step # Use our command
  - turn: Right
  - go: 2
  - plant: Acer
  - go: 4
  - plant: Acer
  - turn: Left
  - *step
  - turn: Left
  - go: 1
  - plant: Oak
  - go: 2
  - plant: Oak
  - turn: Right
  - *step
  - *step
  - plant: Oak
  - turn: Right
  - go: 2
  - plant: Oak
  - turn: Left
  - go: 1
  - turn: Right
  - *step
  - plant: Acer
  - turn: Right
  - turn: Right
  - go: 4
  - plant: Acer
  - turn: Right
  - *step
  - turn: Left
  - *step
  - plant: Birch
  - *step
  - plant: Birch
  - turn: Left
  - turn: Left
  - go: 7
  - plant: Birch
  - *step
  - plant: Birch
  - turn: Right
  - go: 3
  - turn: Right
  - go: 4
"#;

    let cfg: Config = serde_saphyr::from_str(yaml).expect("valid program YAML");

    // Execute the program
    for cmd in cfg.program {
        match cmd {
            Command::Go(n) => robot.go(n),
            Command::Turn(d) => robot.turn(d),
            Command::Plant(opt_tree) => {
                let tree = opt_tree.unwrap_or(Tree::Oak);
                robot.plant(&mut field, tree)
            }
        }
    }

    // Print the resulting field with the robot's final position marked as 'R'
    print_field(&field, robot.x, robot.y);
}
