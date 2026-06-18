let plain () = 1

let if_else x =
  if x > 0 then 1
  else if x < 0 then 2
  else 3

let match_case x =
  match x with
  | 1 -> 1
  | 2 -> 2
  | _ -> 3

let rec loops n =
  let i = ref 0 in
  let s = ref 0 in
  while !i < n do
    s := !s + !i;
    i := !i + 1
  done;
  while !s > 10 do
    s := !s - 1
  done;
  !s

let bool_ops a b c = a && b || c

let ternary x = if x > 0 then 1 else 2

let try_catch x =
  try
    if x then 1 else raise Exit
  with
  | Exit -> 3

let list_comp xs =
  List.filter (fun x -> x > 0) xs

let with_closure x =
  let add a b = if a > b then a else b in
  add x x

let pattern_match opt =
  match opt with
  | Some n -> n
  | None -> 0
