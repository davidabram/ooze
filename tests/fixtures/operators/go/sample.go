package sample

func IsReady(enabled bool) bool {
	if enabled == true {
		return true
	}
	return false
}

func Clamp(x int) int {
	if x < 0 {
		return 0
	}
	if x > 1 {
		return 1
	}
	return x
}

func Both(a bool, b bool) bool {
	return a && b
}
