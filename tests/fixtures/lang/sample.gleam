import gleam/int
import gleam/list

pub fn plain() -> Int {
  1
}

pub fn if_else(x: Int) -> Int {
  case x > 0 {
    True -> 1
    False ->
      case x < 0 {
        True -> 2
        False -> 3
      }
  }
}

pub fn case_demo(x: Int) -> Int {
  case x {
    1 -> 1
    2 -> 2
    _ -> 3
  }
}

pub fn loops(n: Int) -> Int {
  let s = list.range(0, n) |> list.fold(0, fn(acc, i) { acc + i })
  loop_while(s)
}

fn loop_while(s: Int) -> Int {
  case s > 10 {
    True -> loop_while(s - 1)
    False -> s
  }
}

pub fn bool_ops(a: Bool, b: Bool, c: Bool) -> Bool {
  a && b || c
}

pub fn try_catch(x: Bool) -> Int {
  case x {
    True -> 1
    False -> 2
  }
}

pub fn with_closure(x: Int) -> Int {
  let add = fn(a: Int, b: Int) -> Int {
    case a > b {
      True -> a
      False -> b
    }
  }
  add(x, x)
}

pub fn list_comp(xs: List(Int)) -> List(Int) {
  list.filter(xs, fn(x) { x > 0 })
}
