object Sample:
  def plain: Int = 1

  def ifElse(x: Int): Int =
    if x > 0 then 1
    else if x < 0 then 2
    else 3

  def loops(n: Int): Int =
    var s = 0
    for i <- 0 until n do
      s += i
    while s > 10 do
      s -= 1
    s

  def matchCase(x: Int): Int = x match
    case 1 => 1
    case 2 => 2
    case _ => 3

  def ternary(x: Int): Int = if x > 0 then 1 else 2

  def boolOps(a: Boolean, b: Boolean, c: Boolean): Boolean = a && b || c

  def tryCatch(x: Boolean): Int =
    try
      if x then 1 else throw new RuntimeException()
    catch
      case _: Exception => 3

  def listComp(xs: List[Int]): List[Int] = for x <- xs if x > 0 yield x

  def withLambda(x: Int): Int =
    val f = (a: Int) =>
      if a > 0 then a else 0
    f(x)
