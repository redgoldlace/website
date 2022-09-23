---
title = "Dusk musings"
published = 2021-07-11T11:18:00
---

If you've talked to me recently you probably know what Dusk is. I have a bad habit of infecting discussions with
language design woes - of which I have many. If you haven't talked to me lately, I'll do you a favour and give you a
quick explanation: It's a programming language I've been spending the past little while working on. I'm still a bit torn
on how everything will fit together, but I wanted to write this post out as a bit of a rubber-ducking / brainstorming
thing. And I also *really* don't want to forget my ideas in the morning. So here goes!

# Default arguments
This probably doesn't seem so controversial, but if you've used Rust a decent amount I imagine you've probably been hurt
by a lack of them at some point. I'd really like to be able to implement default arguments in a way that works well from
a type-system perspective and isn't a hassle from a usability perspective. Let me explain.  
When it comes to default arguments, we have a couple of options in terms of computing and managing them:
1.  Compute them once for each function, at compile time. This requires all default values be compile-time-evaluable and
    also has potentially surprising consequences if a default value is mutable. Using something innocuous as a default
    (like an empty vector or similar data structure) could lead to strange behaviour if the container is mutated and the
    function is called multiple times.
2.  Compute them once for each function, but do so lazily at runtime - i.e, compute it the first time the function is
    called and requires a default value. This gets rid of the compile-time-evaluable constraint, but adds other
    weirdness. Namely performance could be surprising and behaviour like this should probably generally be opt-in - you
    wouldn't expect to have to compute a default value at runtime, unless you'd knowingly written code to do so. It also
    imposes weird cases where functions will have to somehow store the state of their default values as well as where
    they are - that data has to be stored *somewhere*, and needs to be accessible too.
3.  Compute all default values at call time. This doesn't require that they be compile-time-evaluable, and if something
    like lifetimes or similar are introduced then this works better too. Unfortunately it also has the same issues of
    potentially surprising performance consequences, but doesn't force lazy evaluation on the user.

While I think option 1 would be *nice*, I don't think it's practical. The same goes for option 2, albeit for different 
reasons.  
That leaves us with option 3.

While I haven't figured out a couple of keywords yet, here's an example of what it might look like syntactically:
```rs
fn exponent(first: Int64, second: Int64 = 2) -> Int64 {
    /* This is the original function body */
}
```
You can then call this as `exponent(12)`, `exponent(12, 3)`, or similar.

At a trait/interface level, this could actually be managed fairly elegantly, potentially desugaring to something like
the below:
```rs
struct Fn_exponent;

extend Fn_exponent with Call<Int64> {
    type Return = Int64;

    extern "dusk-fn-call" fn call(self, first: Int64) -> Self::Return {
        let second = 2;
        self(argument, second)
    }
}

extend Fn_exponent with Call<Int64, Int64> {
    type Return = Int64;

    extern "dusk-fn-call" fn call(self, first: Int64, second: Int64) -> Self::Return {
        /* This is the original function body */
    }
}
```
Again, I'm still pretty unsure on keywords here.
One thing I want to note is `Return` being an associated type, rather than being included in the signature for the
interface in the form `Call<Int64, Int64, Int64>` or similar - `Call<Parameters*, Return>` with a Kleene star.
While interfaces like this allow you to use some form of multi-dispatch, defining the same thing multiple times should
still be an error, and ambiguity should be an error too.
if `Call` included the return type in its signature, we might be able to do something like this:
```rs
struct Example;

extend Example with Call<Int64, Int64> {
    //                          ^^^^^ This is the return type
    extern "dusk-fn-call" fn call(self, first: Int64) -> Int64 {
        /* ... */
    }
}

extend Example with Call<Int64, String> {
    //                          ^^^^^^ This is (once again) the return type
    extern "dusk-fn-call" fn call(self, first: Int64) -> String {
        /* ... */
    }
}
```
Consider the following:
```rs
let result = Example(20);
```
What's the type of `result`? If it was being passed to another function, we might be able to infer what we intend the
return type to be. But in this example, we need more context to determine which implementation we use. Leaving this kind
of choice up to the compiler might be a bit strange too - how do we know what implementation it chose? What if the rules
for making that choice change in the future? This of course assumes that we end up choosing an implementation at all. In
this case, it makes no real sense to let the compiler choose. I'm of the opinion that ambiguity in cases like this
should be an error - weird, unsavoury things happen otherwise.

The associated type ensures that the `Call` interface is implemented based on parameter types alone, and has the bonus
of making its return type accessible - that is, `<Example as Call<Int64>>::Return`, or similar. I think it makes it a
bit easier to reason about too.

# Statics and variadics
These are a bit intertwined. Stick with me!

Statics in this case refer to a bundle of a few concepts:
-   The "static context", which refers to the compile-time execution context that encompasses all of this.
-   Static values, which are values that are known (and manipulatable) at compile time.
-   Static functions (which themselves are static values) that operate on other static values.
-   Static logic, which is logic that is "expanded" within a static context - allowing for conditional compilation and
    other similar operations.
-   Static loops, which are loops run inside of a static context that allow for macro-like repetitions of source code or
    similar.

Variadics and statics might not immediately seem related, but consider something innocuous like the below:
```rs
fn print_list<..Ts>(items: Ts) {
    for item in items {
        print(item);
    }
}
```
Seems sensible enough, doesn't it?

Unfortunately, no. Every item of `Ts` is potentially a different type, and this doesn't even express bounds on `Ts` so
that we know we can actually print these items. One option here is to use interface bounds and dynamic dispatch to make
the types known, but that results in another issue - all of these types may be of differing size, so we'd need a heap
allocation or similar if we wanted the stack size to still be statically known.

What other options do we have then?

I'm not very smart, but I think I managed to think of a decent one. In essence, we make variadic type sequences (like
`Ts`) into special "static sequences". These static sequences cannot be iterated outside of a static context because
such a thing would either be nonsensical or break the established laws of reality. In the end, this static iteration
actually ends up looking a bit like a macro-esque or template-esque repetition. Consider something like this:
```rs
fn print_list<..Ts>(items: Ts) {
    static for item in items {
        print(item);
    }
}
```
Say we call this like so:
```rs
print_list("Hello!", 123, 45.3);
```
The variadic is first desugared based on the types of the arguments. If we were writing this out by hand, it might then
look a bit like this:
```rs
fn print_list<..(String, UInt32, Float32)>(items: ..(String, UInt32, Float32)) {
    /* ... */
}
```
Some of this notation is a bit confusing. Just bear with me.  
The next step is flattening this variadic. Now it looks a bit like this:
```rs
fn print_list(items__param_1: String, items__param_2: UInt32, items__param_3: Float32) {
    /* ... */
}
```
You'll notice that the generic type parameter is gone now, and that we've flattened `items` into multiple parameters,
with each taking a type from the variadic as an argument. Their names have been mangled a bit as an example - this
process inherently requires *some* name mangling. If name mangling of parameters becomes an issue in the context of FFI,
then there are certainly ways to address that - but we can cross that bridge when we come to it.

Anyway. In the past few steps, desugaring that `static for` hasn't really made much sense, but it looks a bit more
feasible after this. Let's do that now:
```rs
fn print_list(items__param_1: String, items__param_2: UInt32, items__param_3: Float32) {
    print(items__param_1);
    print(items__param_2);
    print(items__param_3);
}
```
What happens here is actually pretty interesting, and I'm really happy with the concept. It allows for monomorphisation,
inlining and a whole lot of other cool optimisation opportunities. It's also a pretty blatant rip-off of templates with
a nicer coat of paint, but oh well.

Exxcept there's an issue we still haven't solved. Recall the above:
> This doesn't even express bounds on `Ts` so that we know we can actually print these items
This is still an issue, and something we need to figure out. An immediate way of expressing these bounds comes to mind,
and it might look a bit like this:
```rs
fn print_list<..Ts: Show>(items: Ts) {
    static for item in items {
        print(item);
    }
}
```
This might be able to stick around, since it works for simple cases. But what if you need to reference the individual
type in the bound? And what if you need to transform this sequence as part of a return type?  
In those cases, we need something a bit more capable.

Consider something like this:
(Again, remember that syntax here is nowhere near final! And there's still a lot for me to figure out)
```rs
struct Zip<..Ts> {
    iterators: (Ts)
}

extend<..Ts> Zip<..Ts> with Iterator
where
    T: Iterator for T in Ts
{
    type Item = (T::Item for T in Ts);

    fn next(self) -> Maybe<Self::Item> {
        let result = ();

        static for iterator in self.iterators {
            let result = result.push(iterator.next()?);
        }

        Just(result)
    }
}
```
`T: Iterator for T in Ts` is a bound that requires every individual item of the type sequence implement the `Iterator`
interface.
That means that something like this:
```rs
struct Example<..Ts>
where
    T: Clone for T in Ts
{
    blah: (Ts)
}
```
Would roughly desugar into the below when used as `Example<String, UInt32, Float32>`:
```rs
struct Example<Ts__param_1, Ts__param_2, Ts__param_3>
where
    Ts__param_1: Clone,
    Ts__param_2: Clone,
    Ts__param_3: Clone,
{
    blah: (Ts__param_1, Ts__param_2, Ts__param_3)
}
```
The `T::Item for T in Ts` is used to transform a type sequence. It's equivalent to writing out an interface/type/etc.
for each individual type in the type sequence. If you're good at thinking in types, it's roughly equivalent to 
`(A -> B) -> [A] -> [B]` - that is, a function that takes a computation from A to B, a list of items A, and produces a
list of items B. If we take the previous example and change it slightly, it might now look like this:
```rs
struct Example<..Ts>
where
    T: Iterator for T in Ts
{
    blah: (Ts)
}

extend<..Ts> Example<..Ts> {
    fn example(self) -> (T::Item for T in Ts) {
        /* ... */
    }
}
```
Let's once again say that we've used it as `Example<String, UInt32, Float32>`. If we desugar the `example` method as
well, it now looks along the lines of:
```rs
struct Example<Ts__param_1, Ts__param_2, Ts__param_3>
where
    Ts__param_1: Iterator,
    Ts__param_2: Iterator,
    Ts__param_3: Iterator,
{
    blah: (Ts__param_1, Ts__param_2, Ts__param_3)
}

extend<Ts__param_1, Ts__param_2, Ts__param_3> Example<Ts__param_1, Ts__param_2, Ts__param_3> {
    fn example(self) -> (Ts__param_1::Item, Ts__param_2::Item, Ts__param_3::Item) {
        /* ... */
    }
}
```
Which is.. borderline unreadable. But hey! That's why we have variadics and a compiler to write it for us.

# Type system shenanigans
After reading about Typescript a bit, I also had the idea of something along the lines of the below:
```rs
type Parameters<F> where F: Call<..Ts> = Ts;
// This is objectively much less useful.
type Return<F> where F: Call<..Ts> = F::Return;
```
Except I have no clue how `Ts` actually *works* here. It's a strange case where we can only actually name the types once
some precondition that introduces them has been met. This is likely the part where we take yet another page from Rust's
book, and introduce something along the lines of.. higher-rank constructor bounds? I'm not sure what kind of higher-rank
this is, but it's definitely up there.
Maybe that looks like this:
```rs
type Parameters<F> where for<..Ts> F: Call<Ts> = Ts;
type Return<F> where for<..Ts> F: Call<Ts> = F::Return;
```
Though I'm not sure how I feel about this syntactically?
Something like the below might be clearer:
```rs
type Parameters<F> where F: Call<Ts> for all ..Ts = Ts;
type Return<F> where F: Call<Ts> for all ..Ts = F::Return;
```
However I'm not sure about introducing `all` as a keyword.

But don't get me wrong here. I don't know if this is a *good* idea. Are there actually *any* circumstances where you
have something that's `Call<..Ts>`, but can't name `Ts`? And is there any actual circumstance where you can't name the
return type?

I'm not sure

One potential issue I've just thought of is something like this: 
```rs
fn zip_calls<..Fns>(fns: Fns) -> Call<NewArgs, Return=(F::Return for F in Fns)>
where
    F: Call<Args> for all ..Args for F in Fns
{
    /* ... */
}
```
Ignoring the fact that this function is slightly nonsensical, what on earth is `NewArgs`? We actually *don't*
have access to `Ts` here, since it's scoped at the bound level. Speaking of, that bound looks.. pretty ugly. It might be
better to use the (proposed) alternative syntax here:
```rs
fn zip_calls<..Fns>(fns: Fns) -> Call<NewArgs, Return=(F::Return for F in Fns)>
where
    Fns: Call<..Args> for all Args
{
    /* ... */
}
```
But I still don't really like all of this. This "for all" qualification is implicit in other places
(i.e `extend<..Ts> Example<..Ts>`) so I'm not sure if it makes much sense to require it here. So maybe we do something
like this instead:
```rs
fn zip_calls<..Fns>(fns: Fns) -> Call<NewArgs, Return=(F::Return for F in Fns)>
where
    for<..Args> Fns: Call<Args>
{
    /* ... */
}
```
Which is just back to square one, huh?  
We still introduce the type arguments and it's not as much of a nightmare to parse. It doesn't solve our issue though.
And well, are `Parameters` and `Return` really type *synonyms* anymore? They're more of a computation from types to
types. They happen to look similar enough, but.. I'm not sure. I'd like to somehow allow type level computations in
Dusk, but they might not look like this. I have a lot of bikeshedding to do still.

Anyway! I think I might sign off for now. I'd like to write some basics down elsewhere but the above should serve as a
decent little view into all of the terrible ideas I've had that won't come to fruititon.

Thanks for reading this far!
\- Kaylynn