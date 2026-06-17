def plain():
    return 1


def if_elif_else(x):
    if x > 0:
        return 1
    elif x < 0:
        return 2
    else:
        return 3


def loops(items):
    total = 0
    for x in items:
        total += x
    while total > 10:
        total -= 1
    return total


def bool_ops(a, b, c):
    return a and b or c


def ternary(x):
    return 1 if x else 2


def comprehension(xs):
    return [x for x in xs if x > 0]


def match_demo(x):
    match x:
        case 1:
            return 1
        case 2:
            return 2
        case _:
            return 3


def nested():
    def inner(y):
        if y:
            return y
        return 0

    return inner(1)
