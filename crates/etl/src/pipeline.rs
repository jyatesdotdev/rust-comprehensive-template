//! Composable ETL pipeline with trait-based stages.
//!
//! Demonstrates Rust's generics and trait system for type-safe,
//! composable data processing pipelines.

/// A processing stage that transforms input items into output items.
pub trait Stage {
    /// The type this stage consumes.
    type Input;
    /// The type this stage produces.
    type Output;
    /// Process a single item, returning `None` to filter it out.
    fn process(&self, input: Self::Input) -> Option<Self::Output>;
}

/// Chains two stages: `A` feeds into `B`.
pub struct Chain<A, B> {
    first: A,
    second: B,
}

impl<A, B> Stage for Chain<A, B>
where
    A: Stage,
    B: Stage<Input = A::Output>,
{
    type Input = A::Input;
    type Output = B::Output;

    fn process(&self, input: Self::Input) -> Option<Self::Output> {
        self.first.process(input).and_then(|mid| self.second.process(mid))
    }
}

/// Extension trait for composing stages with `.then()`.
pub trait StageExt: Stage + Sized {
    /// Chain this stage with `next`, feeding output into its input.
    fn then<B>(self, next: B) -> Chain<Self, B>
    where
        B: Stage<Input = Self::Output>,
    {
        Chain { first: self, second: next }
    }
}

impl<T: Stage + Sized> StageExt for T {}

/// A stage built from a closure.
pub struct MapStage<F, I, O> {
    f: F,
    _marker: std::marker::PhantomData<fn(I) -> O>,
}

/// Create a stage from a mapping function (always produces output).
pub fn map_stage<I, O, F: Fn(I) -> O>(f: F) -> MapStage<impl Fn(I) -> Option<O>, I, O> {
    MapStage { f: move |input| Some(f(input)), _marker: std::marker::PhantomData }
}

/// Create a stage from a filter-map function (may drop items).
pub fn filter_stage<I, O, F: Fn(I) -> Option<O>>(f: F) -> MapStage<F, I, O> {
    MapStage { f, _marker: std::marker::PhantomData }
}

impl<I, O, F: Fn(I) -> Option<O>> Stage for MapStage<F, I, O> {
    type Input = I;
    type Output = O;
    fn process(&self, input: I) -> Option<O> {
        (self.f)(input)
    }
}

/// Run a pipeline over an iterator, collecting results.
pub fn run<S, I>(stage: &S, data: I) -> Vec<S::Output>
where
    S: Stage,
    I: IntoIterator<Item = S::Input>,
{
    data.into_iter().filter_map(|item| stage.process(item)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_stages() {
        let pipeline = map_stage(|x: i32| x * 2)
            .then(filter_stage(|x: i32| if x > 5 { Some(x) } else { None }))
            .then(map_stage(|x: i32| x.to_string()));

        let result = run(&pipeline, vec![1, 2, 3, 4, 5]);
        assert_eq!(result, vec!["6", "8", "10"]);
    }

    #[test]
    fn filter_drops_items() {
        let stage = filter_stage(|x: i32| if x % 2 == 0 { Some(x) } else { None });
        let result = run(&stage, 1..=6);
        assert_eq!(result, vec![2, 4, 6]);
    }
}
