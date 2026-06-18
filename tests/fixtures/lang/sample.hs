module Sample where

plain :: Int
plain = 1

ifElse :: Int -> Int
ifElse x
    | x > 0     = 1
    | x < 0     = 2
    | otherwise = 3

caseDemo :: Int -> Int
caseDemo x = case x of
    1 -> 1
    2 -> 2
    _ -> 3

loops :: Int -> Int
loops n = sum [0 .. n - 1]

boolOps :: Bool -> Bool -> Bool -> Bool
boolOps a b c = a && b || c

listComp :: [Int] -> [Int]
listComp xs = [x | x <- xs, x > 0]

tryCatch :: Bool -> Int
tryCatch x
    | x         = 1
    | otherwise = 2

patternMatch :: Maybe Int -> Int
patternMatch (Just n) = n
patternMatch Nothing  = 0
