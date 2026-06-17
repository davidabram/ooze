<?php

function plain() {
    return 1;
}

function ifElse($x) {
    if ($x > 0) {
        return 1;
    } elseif ($x < 0) {
        return 2;
    } else {
        return 3;
    }
}

function loops($n) {
    $s = 0;
    for ($i = 0; $i < $n; $i++) {
        $s += $i;
    }
    foreach ([1, 2, 3] as $v) {
        $s += $v;
    }
    while ($s > 10) {
        $s--;
    }
    do {
        $s--;
    } while ($s > 0);
    return $s;
}

function switchCase($x) {
    switch ($x) {
        case 1:
            return 1;
        case 2:
            return 2;
        default:
            return 3;
    }
}

function matchExpr($x) {
    return match ($x) {
        1 => 1,
        2 => 2,
        default => 3,
    };
}

function ternary($x) {
    return $x > 0 ? 1 : 2;
}

function boolOps($a, $b, $c) {
    return $a && $b || $c;
}

function nullCoalesce($a, $b) {
    return $a ?? $b;
}

function tryCatch($x) {
    try {
        if ($x) {
            return 1;
        }
        return 2;
    } catch (Exception $e) {
        return 3;
    }
}

function withClosure($x) {
    $f = function ($a, $b) {
        if ($a > $b) {
            return $a;
        }
        return $b;
    };
    return $f($x, $x);
}
