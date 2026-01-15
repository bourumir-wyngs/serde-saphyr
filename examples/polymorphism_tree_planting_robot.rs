// A tree-planting robot simulation demonstrating how to describe polymorphic commands
// tying them to enums carrying polymorphic fields (distance, direction, tree).
// Robot starts at the center (4,4). It executes an inline program and
// prints the resulting field using characters:
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

use serde::Deserialize;

#[derive(Clone, Copy, Deserialize, Default)]
enum Tree {
    #[default]
    Oak,
    Acer,
    Birch,
}

fn default_birch() -> Tree {
    Tree::Birch
}

#[derive(Clone, Copy, Deserialize)]
enum Direction {
    Left,
    Right,
}

#[derive(Clone, Deserialize)]
enum Command {
    #[serde(rename = "go")]
    Go { distance: usize },
    #[serde(rename = "turn")]
    Turn { direction: Direction },
    // Allow either `plant:` (null/empty) or `plant: { tree: ... }`
    #[serde(rename = "plant")]
    Plant(#[serde(default)] Option<PlantArgs>),
    #[serde(rename = "multistep")]
    MultiStep { steps: Vec<Command> },
}

// Here we complicate a bit to show we can use a structure rather than enum variant
#[derive(Clone, Copy, Deserialize)]
struct PlantArgs {
    #[serde(default = "default_birch")]
    tree: Tree,
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
            if (0..=9).contains(&nx) && (0..=7).contains(&ny) {
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
        // Ensure within bounds (robot should always be within bounds by construction)
        if self.x >= 0 && self.x <= 9 && self.y >= 0 && self.y <= 7 {
            field[self.y as usize][self.x as usize] = ch;
        }
    }
}

fn print_field(field: &[[char; 10]; 8], rx: i32, ry: i32) {
    // Print top (y=7) to bottom (y=0)
    for (y, row) in field.iter().enumerate().rev() {
        let mut line = String::with_capacity(10);
        for (x, ch) in row.iter().enumerate() {
            if x as i32 == rx && y as i32 == ry {
                line.push('R');
            } else {
                line.push(*ch);
            }
        }
        println!("{line}");
    }
}

fn run_program(robot: &mut Robot, field: &mut [[char; 10]; 8], program: &[Command]) {
    for cmd in program.iter().cloned() {
        match cmd {
            Command::Go { distance } => robot.go(distance),
            Command::Turn { direction } => robot.turn(direction),
            Command::Plant(opt) => {
                let tree = opt.unwrap_or(PlantArgs { tree: Tree::Birch }).tree;
                robot.plant(field, tree)
            }
            Command::MultiStep { steps } => {
                // Execute the nested subprogram recursively
                run_program(robot, field, &steps);
            }
        }
    }
}

fn main() {
    // Initialize empty field with dots
    let mut field = [['.'; 10]; 8];
    let mut robot = Robot::new(4, 4);

    // Define a YAML program (externally tagged enum commands as a sequence)
    // Also use some anchors.
    let yaml = r#"
program:
  - go:
      distance: 3
  - turn:
      direction: Left
  - go:
      distance: 4
  - plant:
  - turn: { direction: Right } # Some JSON-like
  - turn:
      direction: Right
  # Let's define a command 'step' as anchor.
  # Anchors must be placed on the same level as the data node they refer to.
  - &step
    go:
      distance: 1
  - plant:
  - go:
      distance: 6
  - plant:
  - *step
  - plant:
  - turn:
      direction: Right
  - *step # Use our command
  - turn:
      direction: Right
  - go:
      distance: 2
  - plant:
      tree: Acer
  - go:
      distance: 4
  - plant:
      tree: Acer
  - turn:
      direction: Left
  - *step
  - turn:
      direction: Left
  - &step_and_oak # Define more complex command: step and then plan an oak.
    multistep:
      steps:
        - go:
            distance: 1
        - plant:
            tree: Oak
  - go:
      distance: 2
  - plant:
      tree: Oak
  - turn:
      direction: Right
  - *step
  - *step_and_oak # Use defined command
  - turn:
      direction: Right
  - go:
      distance: 2
  - plant:
      tree: Oak
  - turn:
      direction: Left
  - go:
      distance: 1
  - turn:
      direction: Right
  - *step
  - plant:
      tree: Acer
  - turn:
      direction: Right
  - turn:
      direction: Right
  - go:
      distance: 4
  - plant:
      tree: Acer
  - turn:
      direction: Right
  - *step
  - turn:
      direction: Left
  - *step
  - plant:
  - *step
  - plant:
      tree: Birch
  - turn:
      direction: Left
  - turn:
      direction: Left
  - go:
      distance: 7
  - plant:
      tree: Birch
  - *step
  - plant:
      tree: Birch
  - turn:
      direction: Right
  - go:
      distance: 3
  - turn:
      direction: Right
  - go:
      distance: 4
"#;

    let cfg: Config = serde_saphyr::from_str(yaml).expect("valid program YAML");

    // Execute the program (supports nested multistep commands)
    run_program(&mut robot, &mut field, &cfg.program);

    // Print the resulting field with the robot's final position marked as 'R'
    print_field(&field, robot.x, robot.y);
}
