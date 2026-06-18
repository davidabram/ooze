int plain() {
  return 1;
}

int ifElse(int x) {
  if (x > 0) {
    return 1;
  } else if (x < 0) {
    return 2;
  } else {
    return 3;
  }
}

int loops(int n) {
  int s = 0;
  for (int i = 0; i < n; i++) {
    s += i;
  }
  while (s > 10) {
    s--;
  }
  do {
    s--;
  } while (s > 0);
  return s;
}

int switchCase(int x) {
  switch (x) {
    case 1:
      return 1;
    case 2:
      return 2;
    default:
      return 3;
  }
}

int switchExpr(int x) {
  return switch (x) {
    1 => 1,
    2 => 2,
    _ => 3,
  };
}

int ternary(int x) {
  return x > 0 ? 1 : 2;
}

bool boolOps(bool a, bool b, bool c) {
  return a && b || c;
}

int tryCatch(bool x) {
  try {
    if (x) {
      return 1;
    }
    throw Exception('fail');
  } catch (e) {
    return 3;
  }
}

int withLambda(int x) {
  var f = (int a) {
    if (a > 0) {
      return a;
    }
    return 0;
  };
  return f(x);
}

List<int> listComp(List<int> xs) {
  return [for (var x in xs) if (x > 0) x];
}

int? nullCoalesce(int? a, int b) {
  return a ?? b;
}
