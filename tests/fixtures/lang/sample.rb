def plain
  1
end

def if_elsif(x)
  if x > 0
    1
  elsif x < 0
    2
  else
    3
  end
end

def unless_demo(x)
  unless x
    1
  end
end

def case_when(x)
  case x
  when 1
    1
  when 2
    2
  else
    3
  end
end

def loops(items)
  total = 0
  for x in items
    total += x
  end
  while total > 10
    total -= 1
  end
  until total == 0
    total -= 1
  end
  total
end

def postfix(x)
  1 if x
end

def bool_ops(a, b, c)
  a && b || c
end

def ternary(x)
  x ? 1 : 2
end

def pattern_match(x)
  case x
  in 1
    1
  in 2
    2
  else
    3
  end
end
