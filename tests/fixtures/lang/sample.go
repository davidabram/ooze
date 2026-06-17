package main

func plain() int {
	return 1
}

func ifElse(x int) int {
	if x > 0 {
		return 1
	} else if x < 0 {
		return 2
	} else {
		return 3
	}
}

func loops(n int) int {
	s := 0
	for i := 0; i < n; i++ {
		s += i
	}
	for s > 10 {
		s--
	}
	return s
}

func switchCase(x int) int {
	switch x {
	case 1:
		return 1
	case 2:
		return 2
	default:
		return 3
	}
}

func typeSwitch(v interface{}) int {
	switch v.(type) {
	case int:
		return 1
	case string:
		return 2
	default:
		return 3
	}
}

func boolOps(a, b, c bool) bool {
	return a && b || c
}

func withClosure(x int) int {
	add := func(a, b int) int {
		if a > b {
			return a
		}
		return b
	}
	return add(x, x)
}
