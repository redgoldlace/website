---
title = "Ranges and suffering"
published = 2021-12-29T20:58:00
---

If you're familiar with Python, you probably like Rust's ranges a lot. They're generally tidy, are lots more concise
than writing out `range(...)` all the time, and are a ton better than magic syntax for slicing (thanks for that one,
Guido)

Unfortunately, the redeeming qualities of Rust's range types stop there. Behind a friendly face lurks what is perhaps
the single biggest collection of infuriating design choices in Rust's entire standard library.

Perhaps you've never run into the issues I'm about to describe. Perhaps you're reading this and asking, "What's wrong
with them?" - after all, they seem fine at first glance, don't they? Surely something so *innocuous*, so *innocent* and
*unimposing* could not possibly be flawed. I would agree - surely not! And yet, to your doubts, your *naïveté*, your
*blissful ignorance* - I laugh. Oh, you sweet summer child! Never to face the despair of the unyielding Rust Stability
Guarantees! You are the the holiest of the holy, the blessed among the wicked! Take leave from this place at once, and
do not return \- the words on the parchment below will taint you forever. You speak of "ranges", but you are yet to see
the face behind the mask! Yield not to the wearer's blasphemous call! Flee, holy one, before you are corrupted and
devoured!

# `Range` is not `Copy`.

Actually, that's a lie. *None* of the 6 range types (Yes, 6) are `Copy`. Why's that? Surely if you
can construct a range comprised of types that are `Copy`, you should be able to copy the range. If you were unaware,
this is how tuples work, which you can see at play below:
```rs
fn assert_copy<T: std::marker::Copy>() {}

fn main() {
    assert_copy::<(usize, usize)>(); // Fine!
    assert_copy::<(Vec<usize>, usize)>(); // Uh oh.
}
```
It would make sense for ranges to use similar behaviour, then. Both can be thought of as a sort of product type, with
the difference being that a range has a little more semantic meaning attached to it.

Unfortunately, no. Range is not *ever* `Copy`, and is instead delegated to only being `Clone`. The reasoning behind this
relates to iterators, and it *makes sense*, but there are other problems involved. I'll explain that one more later.

Because ranges aren't `Copy`, interacting with them inside of a closure (which is common if you're working with
iterators!) is.. nothing short of infuriating. It's normal to need to specify the closure as capturing by `move`,
especially when dealing with closures that make use of a function argument, but ranges aren't content with just that.

At first glance, this probably looks perfectly fine:
```rs
fn eats_a_range(nom: std::ops::Range<usize>) {
    // Say goodbye!
}

fn main() {
    let super_cool_range = 9..27;
    let even_cooler_vec = vec!["Hello", "world!"];
    
    // In the real world, you probably actually care about what's happening inside of `map`.
    // For the sake of example, though, we're gonna pretend the value we get given isn't important.
    
    // `move` causes `super_cool_range` to be moved into the closure, but...
    let foobles = even_cooler_vec.into_iter().map(move |_| eats_a_range(super_cool_range));
    
    // (... do something with `foobles` here)
}
```

Except, you see, Rust closures implement the `Fn` family of traits not based on how they *capture* values, but based on
how they *use* those values. So even though `super_cool_range` is moved into the closure, passing it to `eats_a_range`
turns the closure into a `FnOnce` closure (since `eats_a_range` consumes the range) and means that you can no longer use
it with `map`.

There is a solution to this, but it's ugly. You not only need to specify the closure as capturing by `move`, but *also*
need to clone the range within the closure. If you have more than one range involved, this gets annoying *very* quickly.

# `Range`s are a pain to store in other types

Unfortunately, ranges not being `Copy` makes them a hassle to deal with when they're part of another type.

An adjacent example springs to mind here - `Option` and `Result`. Those types support a lot of adaptor methods to
produce *new* `Option`s or `Result`s as the result of some transformation, and most of those adaptor methods consume
`self`. Naturally, this can make dealing with them inside your own method annoying if *your* method only takes `&self`.

Except, unlike ranges, types like `Option` and `Result` *do* implement `Copy` if their component types allow it. Both of
these types also have an "escape hatch" in the form of the `as_ref` (and co.) methods, which let you turn `&Option<T>`
into `Option<&T>` so that you can proceed as usual. Because of this, they're generally very pleasant types to use and
tend to not cause many issues in the way of ownership.

Unfortunately, ranges are not like that. There is no `Copy`. Your only escape hatch is `Clone`, and you will clone a
*lot*. A small mercy is the fact that you have access to the `start` and `end` fields of `Range<T>`, so if the component
types are `Copy` you can just ignore the hassle that ranges impose.

... except `RangeInclusive<T>` (spelled `a..=b`, instead of the typical `a..b`) doesn't even give you *that* luxury. You
see, the representation of `RangeInclusive` is.. special, so `a..=b` isn't actually equivalent to `a..b+1`, and the
`RangeInclusive` struct has an additional field involved. When you're dealing with inclusive ranges, you now need to use
*methods* to get access to what's contained in the `start` and `end` fields, and those methods return references. This
makes `RangeInclusive` even *more* of a mess than the standard fare range, since you're forced to dereference the result
of `(a..=b).start()` if the range's type is `Copy`, or clone it if it's not.

There's also no way to ask the range to give you *back* its component pieces, so you can't easily create a new range
from an existing one, or reuse an allocation if, say, you've created a range of `Vec`s. It's a black box.

# `Range` is already an iterator

Debugging code that behaves incorrectly because of mutability and copying is genuinely awful - the bugs it causes often
aren't immediately obvious, and you're likely to go digging in the wrong places before you realise what's going on.
Because of that, most people who write Rust code don't let `Copy` types and mutability mix - and for good reason! Ranges
are iterators, and iterators inherently *rely* on mutable state, so ranges *unconditionally* don't implement `Copy`.
So... good choice then, right? Crisis averted?

Except.. ranges shouldn't *be* iterators in the first place. It's useful to iterate *over* them, but forcing them to be
iterators from the get-go comes at an insurmountable ergonomic cost. This decision might make sense if Rust didn't have
any way to get an iterator out of a value, but.. the `IntoIterator` trait exists for this very reason - lots of things
aren't iterators themselves for one reason or another, but *can* be made into an iterator. Vectors, slices, sets - you
name it. All of those types implement `IntoIterator` because you can *make an iterator out of them*, but they don't
*inherently* implement `Iterator` because it just wouldn't make sense. Either those types don't contain the required
state to be iterators, or making them implement `Iterator` **would come at a large cost to ergonomics or performance**.

To their credit, ranges being iterators *does* let you skip the (somewhat annoying) `iter()` or `into_iter()` method
calls, which makes them a bit less onerous to use with iterator adaptors (such as `filter` or `map`) inside of `for`
loops. Yay!

Unfortunately, I don't see this *very* small nicety as worth all of the other issues it causes. The trade-offs just
don't weigh up when you start using a range as more than the subject of a `for` loop.

# `Range` only goes one way

In Rust, a range only contains values if its start is lower than its end. That is, a half-open range of `6..10` contains
`6`, `7`, `8` and `9`, but a range of `10..6` contains nothing, and will yield no values if you attempt to iterate
through it.

You can see that at play here:

```rs
fn main() {
    use std::ops::Range;

    let forwards: Range<usize> = 6..10;
    let backwards: Range<usize> = 10..6;

    assert!(forwards.contains(&8)); // Ok!
    assert!(backwards.is_empty() && !backwards.contains(&8)); // Backwards ranges are always empty
}
```

Like many of the other choices `Range` makes, this makes sense *initially*. Since ranges are often used as index types,
the dogma is that a backwards range usually indicates a logic error; it doesn't really make sense to index a `Vec` with
a backwards range, since slices can only represent a *forwards* range. As such, indexing a `Vec` with a backwards range
will panic.

```rs
fn main() {
    let words = vec!["hello", "brave", "new", "world"];
    let slice = &words[3..1];

    // (... do something with `slice` here)
}
```

Great! This might have just saved us from a bug.

But if you're taking input from somewhere and trying to use *that* as a range, you're in for a rough time. This is
actually something I ran into in this year's [Advent of Code](https://adventofcode.com/2021/day/5). Day 5, to be
specific. I won't go into *too* many details, but I feel that this problem represents a fairly common case where you
might find yourself reaching for a range, only to get bitten by the forwards-only behaviour.

In essence, the problem asks you to take a set of coordinate pairs as input, and draw lines from those coordinates pairs
on a grid. You're then tasked with finding the number of points on the grid where more than *n* lines overlap. We'll
leave that last bit alone - it doesn't involve ranges! - but instead have a look at that first part: Line drawing.

The input for this problem looks a bit like the below, but is customised for each person who participates: 

```
0,9 -> 5,9
8,0 -> 0,8
9,4 -> 3,4
2,2 -> 2,1
7,0 -> 7,4
6,4 -> 2,0
0,9 -> 2,9
3,4 -> 1,4
0,0 -> 8,8
5,5 -> 8,2
```

On the off chance you were wondering, none of the numbers in the input are negative.

If we take the line `0,9 -> 5,9` from that example, the pair of numbers on the *left* side of the arrow (`0,9`)
represents an X position of `0` and a Y position of `9`. Likewise, the pair of numbers on the *right* represents an X
position of `5` and a Y position of `9`. This instructs us to draw a line from X and Y position `(0, 9)` to X and Y
position `(5, 9)`. Simple enough!

Each Advent of Code problem is split into two parts; the first gives you a feel for the problem, and the second usually
builds on it in some way to make it more difficult. For part 1 of this problem, it asks you to only consider *straight*
lines. That is, lines formed by a coordinate pair like `1,1 -> 1,3` or `9,7 -> 7,7`, where both X positions *or* both Y
positions match up. One of the approaches to this problem that might immediately come to mind is to filter out diagonal
lines from your input, and then use a range to iterate over `start..=end`, marking your grid as you go.

So, well, let's do that! We'll start by writing a function that returns an iterator of the coordinates between two
points, and then go from there.

```rs
pub type Point = (usize, usize);

pub fn points_between(start: Point, end: Point) -> impl Iterator<Item = Point> {
    use std::iter::repeat;

    let (start_x, start_y) = start;
    let (end_x, end_y) = end;

    assert!(start_x == end_x || start_y == end_y, "expected a straight line");

    // This is the number of points that the line will occupy. It's necessary so that we can put a length bound on
    // the iterator we return (using `take`), since we don't want argument pairs like `(1, 1)` and `(1, 1)` to exhaust the range and
    // then continue forever because `repeat` is unbounded. Scary stuff!
    let point_count = diff(start_x, end_x).max(diff(start_y, end_y));

    let x = (start_x..=end_x).chain(repeat(end_x).take(point_count));
    let y = (start_y..=end_y).chain(repeat(end_y).take(point_count));

    x.zip(y)
}
```

There are (of course) multiple ways to go about implementing this, but this is the way I decided to go with for our
example here. I've used a type alias of `Point` to make the function signature a bit cleaner, and opted to use
destructuring within the function body instead of the parameter list for the same reason. If you don't know what I mean
when I mention destructuring within parameters, have a look at
[this](https://www.possiblerust.com/guide/how-to-read-rust-functions-part-1). It's a great resource for everything
function-related in Rust!

I've also included a tiny little helper function named `diff` here, since the
[`abs_diff()` method](https://doc.rust-lang.org/std/primitive.usize.html#method.abs_diff) is currently unstable.
It's not scary, and just looks like this:

```rs
use std::ops::Sub;

pub fn diff<T: PartialOrd + Sub>(a: T, b: T) -> <T as Sub>::Output {
    if a > b {
        a - b
    } else {
        b - a
    }
}
```

We use it here to give us the *difference* between two numbers - the minimum distance between them. `diff(5, 10)` and
`diff(10, 5)` are both `5`. Likewise, `diff(5, 7)` and `diff(7, 5)` are both 2.

But, well, let's make sure everything is working as expected! We don't want to get hit by a surprise later and spend a
bunch of time tracking down a bug. To do that, we'll use Rust's very convenient test system.

I've written a little test suite below, and it looks like this:

```rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_points_between() {
        let points: Vec<Point> = points_between((1, 3), (3, 3)).collect();
        let other_direction: Vec<Point> = points_between((3, 1), (3, 3)).collect();

        assert_eq!(points, vec![(1, 3), (2, 3), (3, 3)]);
        assert_eq!(other_direction, vec![(3, 1), (3, 2), (3, 3)]);
    }

    #[test]
    fn test_points_between_terminates() {
        let mut points = points_between((1, 1), (1, 1));

        assert_eq!(points.next(), Some((1, 1)));
        assert_eq!(points.next(), None);
    }

    #[test]
    #[should_panic]
    fn test_invalid_line() {
        // This should panic because of the assertion within the body of the `points_between` function.
        let _points = points_between((5, 1), (1, 5));
    }
}
```

The `#[cfg(test)]` attribute is used for *conditional compilation* - it means that the `tests` module will only be
compiled when we're running tests, and saves us compile time otherwise. Yay!

Then we use the `#[test]` attribute for each function we want to test, add some assertions, and *voilà*! We can run
`cargo test` to make sure everything's working as expected.

```
running 3 tests
test tests::test_points_between ... ok
test tests::test_invalid_line - should panic ... ok
test tests::test_points_between_terminates ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Great! Our tests pass, and that's everything we need to draw our lines, so we're done, right? It's all finished?

Unfortunately, no. **Not at all**. You see, there was a time bomb lurking inside of the example input, silently ticking
away while I carried on about the whimsies of Rust's excellent tooling. Specifically, it's the `9,4 -> 3,4` coordinate
pair. And `2,2 -> 2,1`. And `3,4 -> 1,4` too. Notice a theme?

**All of these coordinate pairs are lines that go backwards**.

Let's write another test, and see what happens when we give it one of these coordinate pairs.

```rs
#[test]
fn test_doom_and_destruction() {
    let points: Vec<Point> = points_between((3, 4), (1, 4)).collect();

    assert_eq!(points, vec![(3, 4), (2, 4), (1, 4)]);
}
```

Running it with `cargo test`.. doesn't go the way we hope.

```
running 4 tests
test tests::test_points_between_terminates ... ok
test tests::test_points_between ... ok
test tests::test_invalid_line - should panic ... ok
test tests::test_doom_and_destruction ... FAILED

failures:

---- tests::test_doom_and_destruction stdout ----
thread 'tests::test_doom_and_destruction' panicked at 'assertion failed: `(left == right)`
  left: `[(1, 4), (1, 4)]`,
 right: `[(3, 4), (2, 4), (1, 4)]`', src\main.rs:68:9


failures:
    tests::test_doom_and_destruction

test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Behold! The treachery of The Range rears its ugly head, swallowing another soul and tearing a legitimate use-case
asunder!

Because we used a range as something *slightly more* than the subject of a `for` loop, a piece of code that initially
appeared to work perfectly sputters and dies on a certain class of inputs. And yes, this behaviour is documented.
Granted, that documentation is placed in the abyss of `std::ops`, but it *is* documented. Regardless of that, it's still
incredibly easy to forget about such behaviour when writing code, or gloss over it in a code review. It doesn't help
that the bugs caused by backwards ranges being empty are often *absolutely vexing*. A range like `start_x..=end_x` is
*always* the last place you look when you start to encounter spurious failures somewhere in a larger program or system.

Because, after all, ranges are *innocent*, *innocuous*, and *unimposing* types.

# You can't go both ways

Let's forgive all of the other issues with Rust ranges for just a moment, and try to get around the issue of backwards
ranges.

Iterators have a `rev()` method that we can use to iterate in reverse order, and ranges are iterators, so let's start
there and try our hand at writing a range adaptor. That should be simple enough!

```rs
use std::ops::Range;

pub fn range(start: usize, stop: usize) -> Range<usize> {
    if start > stop {
        (stop..start).rev()
    } else {
        start..stop
    }
}
```

Unfortunately, we can't write a generic version of this function because the trait used by the range types under the
hood ([Step](https://doc.rust-lang.org/std/iter/trait.Step.html)) is unstable, so for this example we'll just make the
function deal in `usize` values.

If we try running this, though, we find ourselves at a bit of an impasse:

```
error[E0308]: mismatched types
 --> src/lib.rs:5:9
  |
3 | pub fn range(start: usize, stop: usize) -> Range<usize> {
  |                                            ------------ expected `std::ops::Range<usize>` because of return type
4 |     if start > stop {
5 |         (stop..start).rev()
  |         ^^^^^^^^^^^^^^^^^^^ expected struct `std::ops::Range`, found struct `Rev`
  |
  = note: expected struct `std::ops::Range<_>`
             found struct `Rev<std::ops::Range<_>>`
```

This is because the `rev()` method wraps the original iterator in its own type, which causes the two branches of that
`if` expression to have different types. So, with the obvious option out of the way, what can we do instead?

## \#1: Dynamic dispatch

A simple solution is to give in to The Box, and use dynamic dispatch.

```rs
pub fn range(start: usize, stop: usize) -> Box<dyn Iterator<Item = usize>> {
    if start > stop {
        Box::new((stop..start).rev())
    } else {
        Box::new(start..stop)
    }
}
```

**This is by no means an ideal solution**. Dynamic dispatch prevents inlining, and forces us to allocate using a `Box`
since all `dyn Trait` types are *unsized* - they don't have a size known at compile time. Using a `Box` sidesteps this
by allocating the value on the *heap* instead of the *stack*, which gives you flexibility at the cost of performance. If
you'd like to read more about boxes - the Rust kind, not the "I'm moving house" kind - I'd suggest having a look
[here](https://fasterthanli.me/articles/whats-in-the-box). Thanks Amos!

## \#2: Writing our own type

So we *can* do dynamic dispatch, but what if we want to do better? We can write our own iterator adaptor that wraps
around forward and backwards iterators, and then throw a range into it.

Welp. Time to bite the bullet.

```rs
use std::iter::Rev;

pub enum Bidirectional<I> {
    Forward(I),
    Reversed(Rev<I>),
}

impl<I: DoubleEndedIterator> Iterator for Bidirectional<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<I::Item> {
        match self {
            Bidirectional::Forward(iter) => iter.next(),
            Bidirectional::Reversed(iter) => iter.next(),
        }
    }
}

impl<I: DoubleEndedIterator> DoubleEndedIterator for Bidirectional<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Bidirectional::Forward(iter) => iter.next_back(),
            Bidirectional::Reversed(iter) => iter.next_back(),
        }
    }
}
```

This is boilerplate-y and a bit gross, and there are absolutely nicer ways to do it, but It Works™ and the compiler can
inline it within an inch of its life. If we shove it into a dark hole somewhere where it'll never see the light of day
again, it's probably fine.

Anyway. Let's plug it into our `range` function:

```rs
pub fn range(start: usize, stop: usize) -> Bidirectional<Range<usize>> {
    if start > stop {
        Bidirectional::Reversed((stop..start).rev())
    } else {
        Bidirectional::Forward(start..stop)
    }
}
```

Great! Now the types match up and everybody's happy.

## A comparison

Unfortunately, neither of these approaches are particularly great. Dynamic dispatch is very simple, but comes at the cost of
performance and the loss of type information. Writing our own adaptor is more performant, but it's not particularly
*nice* code, and something that you would probably want to factor out into a dependency if you noticed it getting
reused across projects. Good iterator implementations can be pretty involved too, so making that `Bidirectional` type a
bit more flexible would likely result in a decent chunk of extra code.

Ultimately, both of these are solutions to a **problem that doesn't need to exist**, which is what really upsets me.
**This is not code we would need to write if Rust's range types were more sophisticated**.

So.. what now?

# Where to from here

Solving the issues with Rust's ranges is.. actually quite frustrating, mostly because of stability guarantees. Nearly
anything that would improve the type's ergonomics would thoroughly massacre existing code. As such, for a mere moment,
let us throw stability to the wind, and envision what an *ideal* range type might look like.

- An ideal range type implements `Copy` if its component types allow it. This follows the behaviour of other types
  already present in the standard library, such as `Option`, `Result`, and the many varieties of tuple.
- Adjacently, an ideal range type is *not* an iterator in and of itself, and implements `IntoIterator` instead. This
  comes at a small hit to ergonomics when using a range with iterator adaptors, but **does more good than bad** since it
  allows the type to implement `Copy` without causing other issues.
- An ideal range type understands directions. This is somewhat more involved, since storing or computing a direction has
  a (mostly small, but nonetheless notable) impact on the size or performance of the type. Existing types that allow the
  usage of ranges for indexing operations (such as `String` and `Vec`) would be mostly unaffected by this change, since
  they already panic when a backwards range is supplied. Importantly, this change would allow you to iterate through a
  range regardless of its direction, and not require allocations or iterator hacks when a range's bounds aren't
  statically known.
- An ideal range type.. probably isn't 6 of them. One of the ideas I've toyed with is the idea of coalescing the current
  range types into prospective `Range`, `RangeOpen` and `RangeFull` types, with `Range` taking the place of the current
  `Range` + `RangeInclusive` types, `RangeOpen` taking the place of `RangeFrom` + `RangeTo` + `RangeToInclusive`, and
  `RangeFull` remaining largely the same. There are issues with the logistics of such a change, though, so I'm not too
  sold on yet, but it *is* something to keep in mind.
- An ideal range type has more than 2 methods. Specifically, it would be useful for methods like `intersection`,
  `union`, `is_subset`, and `is_superset` to be implemented if the range's component type was `Ord` or `PartialOrd`.

Ultimately, Rust's existing range types suffer from offering too little and being too rigid. They don't cover legitimate
use cases that you could reasonably expect somebody to want a range for, and are generally so much of a hassle to use
that people wind up using different types or reimplementing existing functionality in a different manner. **They're just
not sufficient**. There are solutions to these problems - and I've described how I think we might be able to solve some
of them - but in the face of Rust's stability guarantees, I don't know if making meaningful changes is feasible.

# And then, an epilogue

It's important for me to mention that this isn't an RFC, or anything near official. It's simply a transcription of my
own spite-fuelled ramblings! Despite that, I would really love to hear what you thought about this blog post, and what
you think about Rust's range types in general. I wrote this to detail the annoyances I've run into in the wild, and I
hope that reading all of this has been informative, or (at the very least) has given you something to think about.

By the same token, I really hope that there can be some meaningful changes made. Things have been hard recently, so I
don't know if I'd be up to doing it myself, but it would be great to see an RFC or some other more focused improvements
in this space.

On a more personal note, this sprawling mess of a blog post is *done*. I've spent far too many hours writing this, and I
just wanna go play games now. Thanks for sticking with me, and well - happy new year!

Cheers,
\- Kaylynn