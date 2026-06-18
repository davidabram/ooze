defmodule Sample do

  def plain do
    1
  end

  def if_else do
    x = 0
    if x > 0 do
      1
    else
      if x < 0 do
        2
      else
        3
      end
    end
  end

  def cond_demo do
    x = 0
    cond do
      x > 0 -> 1
      x < 0 -> 2
      true  -> 3
    end
  end

  def case_demo do
    x = 0
    case x do
      1 -> 1
      2 -> 2
      _ -> 3
    end
  end

  def loops do
    s = 20
    if s > 10 do
      s - 1
    else
      s
    end
  end

  def bool_ops do
    a = true
    b = true
    c = false
    a && b || c
  end

  def try_catch do
    x = true
    try do
      case x do
        true  -> 1
        false -> throw(:error)
      end
    rescue
      _ -> 3
    end
  end

  def list_comp do
    xs = [1, 2, 3]
    for x <- xs, x > 0, do: x
  end

  def with_closure do
    x = 1
    add = fn a, b ->
      if a > b, do: a, else: b
    end
    add.(x, x)
  end

end
