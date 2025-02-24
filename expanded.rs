
module_path:
"task"

module_path:
"workflow"

machine_info:
MachineInfo {
    name: "Machine",
    vis: "pub",
    derives: [
        "Debug",
    ],
    fields: [],
    module_path: MachinePath(
        "workflow",
    ),
    generics: "< State >",
}
#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use statum::{machine, state, transition};
pub mod task {
    use super::*;
    pub trait DoesNotRequireStateData {}
    pub trait RequiresStateData {}
    pub enum State {
        Idle,
        Running,
        Completed,
    }
    pub trait StateTrait {
        type Data;
    }
    pub struct Idle;
    #[automatically_derived]
    impl ::core::fmt::Debug for Idle {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Idle")
        }
    }
    impl StateTrait for Idle {
        type Data = ();
    }
    impl DoesNotRequireStateData for Idle {}
    pub struct Running;
    #[automatically_derived]
    impl ::core::fmt::Debug for Running {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Running")
        }
    }
    impl StateTrait for Running {
        type Data = ();
    }
    impl DoesNotRequireStateData for Running {}
    pub struct Completed;
    #[automatically_derived]
    impl ::core::fmt::Debug for Completed {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Completed")
        }
    }
    impl StateTrait for Completed {
        type Data = ();
    }
    impl DoesNotRequireStateData for Completed {}
    pub struct UninitializedState;
    impl StateTrait for UninitializedState {
        type Data = ();
    }
}
pub mod workflow {
    use super::*;
    pub trait DoesNotRequireStateData {}
    pub trait RequiresStateData {}
    pub enum State {
        NotStarted,
        InProgress(TaskMachine<Running>),
        Finished,
    }
    pub trait StateTrait {
        type Data;
    }
    pub struct NotStarted;
    #[automatically_derived]
    impl ::core::fmt::Debug for NotStarted {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "NotStarted")
        }
    }
    impl StateTrait for NotStarted {
        type Data = ();
    }
    impl DoesNotRequireStateData for NotStarted {}
    pub struct InProgress(pub TaskMachine<Running>);
    #[automatically_derived]
    impl ::core::fmt::Debug for InProgress {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_tuple_field1_finish(f, "InProgress", &&self.0)
        }
    }
    impl StateTrait for InProgress {
        type Data = TaskMachine<Running>;
    }
    impl RequiresStateData for InProgress {}
    pub struct Finished;
    #[automatically_derived]
    impl ::core::fmt::Debug for Finished {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Finished")
        }
    }
    impl StateTrait for Finished {
        type Data = ();
    }
    impl DoesNotRequireStateData for Finished {}
    pub struct UninitializedState;
    impl StateTrait for UninitializedState {
        type Data = ();
    }
    pub trait TransitionTo<N: StateTrait> {
        fn transition(self) -> Machine<N>;
    }
    pub trait TransitionWith<T> {
        type NextState: StateTrait;
        fn transition_with(self, data: T) -> Machine<Self::NextState>;
    }
    pub struct Machine<S: StateTrait = UninitializedState> {
        marker: core::marker::PhantomData<S>,
        pub state_data: S::Data,
    }
    #[automatically_derived]
    impl<S: ::core::fmt::Debug + StateTrait> ::core::fmt::Debug for Machine<S>
    where
        S::Data: ::core::fmt::Debug,
    {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field2_finish(
                f,
                "Machine",
                "marker",
                &self.marker,
                "state_data",
                &&self.state_data,
            )
        }
    }
    impl Machine<NotStarted> {
        #[doc(hidden)]
        #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
        fn __orig_new() -> Machine<NotStarted> {
            Machine {
                marker: core::marker::PhantomData,
                state_data: (),
            }
        }
        #[inline(always)]
        #[allow(clippy::inline_always, clippy::use_self, clippy::missing_const_for_fn)]
        #[allow(deprecated)]
        pub fn builder() -> NotStartedBuilder {
            NotStartedBuilder {
                __unsafe_private_phantom: ::core::marker::PhantomData,
                __unsafe_private_named: ::core::default::Default::default(),
            }
        }
    }
    #[allow(unnameable_types, unreachable_pub, clippy::redundant_pub_crate)]
    /**Tools for manipulating the type state of [`NotStartedBuilder`].

See the [detailed guide](https://bon-rs.com/guide/typestate-api) that describes how all the pieces here fit together.*/
    #[allow(deprecated)]
    mod notstarted_builder {
        #[doc(inline)]
        pub use ::statum::bon::__::{IsSet, IsUnset};
        use ::statum::bon::__::{Set, Unset};
        mod sealed {
            pub struct Sealed;
        }
        ///Builder's type state specifies if members are set or not (unset).
        pub trait State: ::core::marker::Sized {
            #[doc(hidden)]
            const SEALED: sealed::Sealed;
        }
        /**Marker trait that indicates that all required members are set.

In this state, you can finish building by calling the method [`NotStartedBuilder::build()`](super::NotStartedBuilder::build())*/
        pub trait IsComplete: State {
            #[doc(hidden)]
            const SEALED: sealed::Sealed;
        }
        #[doc(hidden)]
        impl<S: State> IsComplete for S {
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
        #[deprecated = "this should not be used directly; it is an implementation detail; use the Set* type aliases to control the state of members instead"]
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        mod members {}
        /// Represents a [`State`] that has [`IsUnset`] implemented for all members.
        ///
        /// This is the initial state of the builder before any setters are called.
        pub struct Empty(());
        #[doc(hidden)]
        impl State for Empty {
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
    }
    #[must_use = "the builder does nothing until you call `build()` on it to finish building"]
    ///Use builder syntax to set the inputs and finish with [`build()`](Self::build()).
    #[allow(unused_parens)]
    #[allow(clippy::struct_field_names, clippy::type_complexity)]
    #[allow(deprecated)]
    pub struct NotStartedBuilder<
        S: notstarted_builder::State = notstarted_builder::Empty,
    > {
        #[doc(hidden)]
        #[deprecated = "this field should not be used directly; it's an implementation detail, and if you access it directly, you may break some internal unsafe invariants; if you found yourself needing it, then you are probably doing something wrong; feel free to open an issue/discussion in our GitHub repository (https://github.com/elastio/bon) or ask for help in our Discord server (https://bon-rs.com/discord)"]
        __unsafe_private_phantom: ::core::marker::PhantomData<
            (fn() -> S, fn() -> ::core::marker::PhantomData<Machine<NotStarted>>),
        >,
        #[doc(hidden)]
        #[deprecated = "this field should not be used directly; it's an implementation detail, and if you access it directly, you may break some internal unsafe invariants; if you found yourself needing it, then you are probably doing something wrong; feel free to open an issue/discussion in our GitHub repository (https://github.com/elastio/bon) or ask for help in our Discord server (https://bon-rs.com/discord)"]
        __unsafe_private_named: (),
    }
    #[allow(unused_parens)]
    #[automatically_derived]
    #[allow(deprecated)]
    impl<S: notstarted_builder::State> NotStartedBuilder<S> {
        /// Finishes building and performs the requested action.
        #[inline(always)]
        #[allow(
            clippy::inline_always,
            clippy::future_not_send,
            clippy::missing_const_for_fn,
        )]
        pub fn build(self) -> Machine<NotStarted>
        where
            S: notstarted_builder::IsComplete,
        {
            <Machine<NotStarted>>::__orig_new()
        }
    }
    impl Machine<InProgress> {
        #[doc(hidden)]
        #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
        fn __orig_new(state_data: TaskMachine<Running>) -> Machine<InProgress> {
            Machine {
                marker: core::marker::PhantomData,
                state_data,
            }
        }
        #[inline(always)]
        #[allow(clippy::inline_always, clippy::use_self, clippy::missing_const_for_fn)]
        #[allow(deprecated)]
        pub fn builder() -> InProgressBuilder {
            InProgressBuilder {
                __unsafe_private_phantom: ::core::marker::PhantomData,
                __unsafe_private_named: ::core::default::Default::default(),
            }
        }
    }
    #[allow(unnameable_types, unreachable_pub, clippy::redundant_pub_crate)]
    /**Tools for manipulating the type state of [`InProgressBuilder`].

See the [detailed guide](https://bon-rs.com/guide/typestate-api) that describes how all the pieces here fit together.*/
    #[allow(deprecated)]
    mod inprogress_builder {
        #[doc(inline)]
        pub use ::statum::bon::__::{IsSet, IsUnset};
        use ::statum::bon::__::{Set, Unset};
        mod sealed {
            pub struct Sealed;
        }
        /**Builder's type state specifies if members are set or not (unset).

You can use the associated types of this trait to control the state of individual members with the [`IsSet`] and [`IsUnset`] traits. You can change the state of the members with the `Set*` structs available in this module.*/
        pub trait State: ::core::marker::Sized {
            /**Type state of the member `state_data`.

It can implement either [`IsSet`] or [`IsUnset`]*/
            type StateData;
            #[doc(hidden)]
            const SEALED: sealed::Sealed;
        }
        /**Marker trait that indicates that all required members are set.

In this state, you can finish building by calling the method [`InProgressBuilder::build()`](super::InProgressBuilder::build())*/
        pub trait IsComplete: State<StateData: IsSet> {
            #[doc(hidden)]
            const SEALED: sealed::Sealed;
        }
        #[doc(hidden)]
        impl<S: State> IsComplete for S
        where
            S::StateData: IsSet,
        {
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
        #[deprecated = "this should not be used directly; it is an implementation detail; use the Set* type aliases to control the state of members instead"]
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        mod members {
            pub struct state_data(());
        }
        /// Represents a [`State`] that has [`IsUnset`] implemented for all members.
        ///
        /// This is the initial state of the builder before any setters are called.
        pub struct Empty(());
        /**Represents a [`State`] that has [`IsSet`] implemented for [`State::StateData`].

The state for all other members is left the same as in the input state.*/
        pub struct SetStateData<S: State = Empty>(
            ::core::marker::PhantomData<fn() -> S>,
        );
        #[doc(hidden)]
        impl State for Empty {
            type StateData = Unset<members::state_data>;
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
        #[doc(hidden)]
        impl<S: State> State for SetStateData<S> {
            type StateData = Set<members::state_data>;
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
    }
    #[must_use = "the builder does nothing until you call `build()` on it to finish building"]
    ///Use builder syntax to set the inputs and finish with [`build()`](Self::build()).
    #[allow(unused_parens)]
    #[allow(clippy::struct_field_names, clippy::type_complexity)]
    #[allow(deprecated)]
    pub struct InProgressBuilder<
        S: inprogress_builder::State = inprogress_builder::Empty,
    > {
        #[doc(hidden)]
        #[deprecated = "this field should not be used directly; it's an implementation detail, and if you access it directly, you may break some internal unsafe invariants; if you found yourself needing it, then you are probably doing something wrong; feel free to open an issue/discussion in our GitHub repository (https://github.com/elastio/bon) or ask for help in our Discord server (https://bon-rs.com/discord)"]
        __unsafe_private_phantom: ::core::marker::PhantomData<
            (fn() -> S, fn() -> ::core::marker::PhantomData<Machine<InProgress>>),
        >,
        #[doc(hidden)]
        #[deprecated = "this field should not be used directly; it's an implementation detail, and if you access it directly, you may break some internal unsafe invariants; if you found yourself needing it, then you are probably doing something wrong; feel free to open an issue/discussion in our GitHub repository (https://github.com/elastio/bon) or ask for help in our Discord server (https://bon-rs.com/discord)"]
        __unsafe_private_named: (::core::option::Option<TaskMachine<Running>>,),
    }
    #[allow(unused_parens)]
    #[automatically_derived]
    #[allow(deprecated)]
    impl<S: inprogress_builder::State> InProgressBuilder<S> {
        /// Finishes building and performs the requested action.
        #[inline(always)]
        #[allow(
            clippy::inline_always,
            clippy::future_not_send,
            clippy::missing_const_for_fn,
        )]
        pub fn build(self) -> Machine<InProgress>
        where
            S: inprogress_builder::IsComplete,
        {
            let state_data: TaskMachine<Running> = unsafe {
                ::core::option::Option::unwrap_unchecked(self.__unsafe_private_named.0)
            };
            <Machine<InProgress>>::__orig_new(state_data)
        }
        /**_**Required.**_

*/
        #[allow(
            clippy::inline_always,
            clippy::impl_trait_in_params,
            clippy::missing_const_for_fn,
        )]
        #[inline(always)]
        pub fn state_data(
            mut self,
            value: TaskMachine<Running>,
        ) -> InProgressBuilder<inprogress_builder::SetStateData<S>>
        where
            S::StateData: inprogress_builder::IsUnset,
        {
            self.__unsafe_private_named.0 = ::core::option::Option::Some(value);
            InProgressBuilder {
                __unsafe_private_phantom: ::core::marker::PhantomData,
                __unsafe_private_named: self.__unsafe_private_named,
            }
        }
    }
    impl Machine<Finished> {
        #[doc(hidden)]
        #[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
        fn __orig_new() -> Machine<Finished> {
            Machine {
                marker: core::marker::PhantomData,
                state_data: (),
            }
        }
        #[inline(always)]
        #[allow(clippy::inline_always, clippy::use_self, clippy::missing_const_for_fn)]
        #[allow(deprecated)]
        pub fn builder() -> FinishedBuilder {
            FinishedBuilder {
                __unsafe_private_phantom: ::core::marker::PhantomData,
                __unsafe_private_named: ::core::default::Default::default(),
            }
        }
    }
    #[allow(unnameable_types, unreachable_pub, clippy::redundant_pub_crate)]
    /**Tools for manipulating the type state of [`FinishedBuilder`].

See the [detailed guide](https://bon-rs.com/guide/typestate-api) that describes how all the pieces here fit together.*/
    #[allow(deprecated)]
    mod finished_builder {
        #[doc(inline)]
        pub use ::statum::bon::__::{IsSet, IsUnset};
        use ::statum::bon::__::{Set, Unset};
        mod sealed {
            pub struct Sealed;
        }
        ///Builder's type state specifies if members are set or not (unset).
        pub trait State: ::core::marker::Sized {
            #[doc(hidden)]
            const SEALED: sealed::Sealed;
        }
        /**Marker trait that indicates that all required members are set.

In this state, you can finish building by calling the method [`FinishedBuilder::build()`](super::FinishedBuilder::build())*/
        pub trait IsComplete: State {
            #[doc(hidden)]
            const SEALED: sealed::Sealed;
        }
        #[doc(hidden)]
        impl<S: State> IsComplete for S {
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
        #[deprecated = "this should not be used directly; it is an implementation detail; use the Set* type aliases to control the state of members instead"]
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        mod members {}
        /// Represents a [`State`] that has [`IsUnset`] implemented for all members.
        ///
        /// This is the initial state of the builder before any setters are called.
        pub struct Empty(());
        #[doc(hidden)]
        impl State for Empty {
            const SEALED: sealed::Sealed = sealed::Sealed;
        }
    }
    #[must_use = "the builder does nothing until you call `build()` on it to finish building"]
    ///Use builder syntax to set the inputs and finish with [`build()`](Self::build()).
    #[allow(unused_parens)]
    #[allow(clippy::struct_field_names, clippy::type_complexity)]
    #[allow(deprecated)]
    pub struct FinishedBuilder<S: finished_builder::State = finished_builder::Empty> {
        #[doc(hidden)]
        #[deprecated = "this field should not be used directly; it's an implementation detail, and if you access it directly, you may break some internal unsafe invariants; if you found yourself needing it, then you are probably doing something wrong; feel free to open an issue/discussion in our GitHub repository (https://github.com/elastio/bon) or ask for help in our Discord server (https://bon-rs.com/discord)"]
        __unsafe_private_phantom: ::core::marker::PhantomData<
            (fn() -> S, fn() -> ::core::marker::PhantomData<Machine<Finished>>),
        >,
        #[doc(hidden)]
        #[deprecated = "this field should not be used directly; it's an implementation detail, and if you access it directly, you may break some internal unsafe invariants; if you found yourself needing it, then you are probably doing something wrong; feel free to open an issue/discussion in our GitHub repository (https://github.com/elastio/bon) or ask for help in our Discord server (https://bon-rs.com/discord)"]
        __unsafe_private_named: (),
    }
    #[allow(unused_parens)]
    #[automatically_derived]
    #[allow(deprecated)]
    impl<S: finished_builder::State> FinishedBuilder<S> {
        /// Finishes building and performs the requested action.
        #[inline(always)]
        #[allow(
            clippy::inline_always,
            clippy::future_not_send,
            clippy::missing_const_for_fn,
        )]
        pub fn build(self) -> Machine<Finished>
        where
            S: finished_builder::IsComplete,
        {
            <Machine<Finished>>::__orig_new()
        }
    }
}
fn main() {
    let task_machine = TaskMachine::<Running>::builder().build();
    let workflow_machine = workflow::Machine::<workflow::NotStarted>::builder().build();
    let workflow_machine = workflow_machine.start(task_machine);
    {
        ::std::io::_print(
            format_args!("Task State: {0:?}\n", &workflow_machine.state_data),
        );
    };
}
