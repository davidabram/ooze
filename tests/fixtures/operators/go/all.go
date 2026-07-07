package sample

func swapBoolean(enabled bool) bool {
	if enabled {
		return true
	}
	return enabled
}

func negateEquality(a int, b int) bool {
	return a == b
}

func compare(a int, b int) bool {
	return a < b
}

func swapLogical(x bool, y bool) bool {
	return x && y
}

func integerZeroOne(n int) int {
	if n > 5 {
		return 0
	}
	return n
}
