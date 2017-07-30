// Copyright (c) 2017 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use VulkanObject;
use buffer::BufferAccess;
use command_buffer::DynamicState;
use descriptor::DescriptorSet;
use pipeline::input_assembly::IndexType;
use pipeline::ComputePipelineAbstract;
use pipeline::GraphicsPipelineAbstract;
use smallvec::SmallVec;
use vk;

/// Keep track of the state of a command buffer builder, so that you don't need to bind objects
/// that were already bound.
///
/// > **Important**: Executing a secondary command buffer invalidates the state of a command buffer
/// > builder. When you do so, you need to call `invalidate()`.
pub struct StateCacher {
    // The dynamic state to synchronize with `CmdSetState`.
    dynamic_state: DynamicState,
    // The compute pipeline currently bound. 0 if nothing bound.
    compute_pipeline: vk::Pipeline,
    // The graphics pipeline currently bound. 0 if nothing bound.
    graphics_pipeline: vk::Pipeline,
    // The descriptor sets for the compute pipeline.
    compute_descriptor_sets: SmallVec<[vk::DescriptorSet; 12]>,
    // The descriptor sets for the graphics pipeline.
    graphics_descriptor_sets: SmallVec<[vk::DescriptorSet; 12]>,
    // If the user starts comparing descriptor sets, but drops the helper struct in the middle of
    // the processing then we will end up in a weird state. This bool is true when we start
    // comparing sets, and is set to false when we end up comparing. If it was true when we start
    // comparing, we know that something bad happened and we flush the cache.
    poisonned_descriptor_sets: bool,
    // The index buffer, offset, and index type currently bound. `None` if nothing bound.
    index_buffer: Option<(vk::Buffer, usize, IndexType)>,
}

/// Outcome of an operation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StateCacherOutcome {
    /// The caller needs to perform the state change in the actual command buffer builder.
    NeedChange,
    /// The state change is not necessary.
    AlreadyOk,
}

impl StateCacher {
    /// Builds a new `StateCacher`.
    #[inline]
    pub fn new() -> StateCacher {
        StateCacher {
            dynamic_state: DynamicState::none(),
            compute_pipeline: 0,
            graphics_pipeline: 0,
            compute_descriptor_sets: SmallVec::new(),
            graphics_descriptor_sets: SmallVec::new(),
            poisonned_descriptor_sets: false,
            index_buffer: None,
        }
    }

    /// Resets the cache to its default state. You **must** call this after executing a secondary
    /// command buffer.
    #[inline]
    pub fn invalidate(&mut self) {
        self.dynamic_state = DynamicState::none();
        self.compute_pipeline = 0;
        self.graphics_pipeline = 0;
        self.compute_descriptor_sets = SmallVec::new();
        self.graphics_descriptor_sets = SmallVec::new();
        self.index_buffer = None;
    }

    /// Compares the current state with `incoming`, and returns a new state that contains the
    /// states that differ and that need to be actually set in the command buffer builder.
    ///
    /// This function also updates the state cacher. The state cacher assumes that the state
    /// changes are going to be performed after this function returns.
    pub fn dynamic_state(&mut self, mut incoming: DynamicState) -> DynamicState {
        macro_rules! cmp {
            ($field:ident) => (
                if self.dynamic_state.$field == incoming.$field {
                    incoming.$field = None;
                } else if incoming.$field.is_some() {
                    self.dynamic_state.$field = incoming.$field.clone();
                }
            );
        }

        cmp!(line_width);
        cmp!(viewports);
        cmp!(scissors);

        incoming
    }

    /// Starts the process of comparing a list of descriptor sets to the descriptor sets currently
    /// in cache.
    ///
    /// After calling this function, call `add` for each set one by one. Then call `compare` in
    /// order to get the index of the first set to bind, or `None` if the sets were identical to
    /// what is in cache.
    ///
    /// This process also updates the state cacher. The state cacher assumes that the state
    /// changes are going to be performed after the `compare` function returns.
    #[inline]
    pub fn bind_descriptor_sets(&mut self, graphics: bool) -> StateCacherDescriptorSets {
        if self.poisonned_descriptor_sets {
            self.compute_descriptor_sets = SmallVec::new();
            self.graphics_descriptor_sets = SmallVec::new();
        }

        self.poisonned_descriptor_sets = true;

        StateCacherDescriptorSets {
            poisonned: &mut self.poisonned_descriptor_sets,
            state: if graphics {
                &mut self.graphics_descriptor_sets
            } else {
                &mut self.compute_descriptor_sets
            },
            offset: 0,
            found_diff: None,
        }
    }

    /// Checks whether we need to bind a graphics pipeline. Returns `StateCacherOutcome::AlreadyOk`
    /// if the pipeline was already bound earlier, and `StateCacherOutcome::NeedChange` if you need
    /// to actually bind the pipeline.
    ///
    /// This function also updates the state cacher. The state cacher assumes that the state
    /// changes are going to be performed after this function returns.
    pub fn bind_graphics_pipeline<P>(&mut self, pipeline: &P) -> StateCacherOutcome
        where P: GraphicsPipelineAbstract
    {
        let inner = GraphicsPipelineAbstract::inner(pipeline).internal_object();
        if inner == self.graphics_pipeline {
            StateCacherOutcome::AlreadyOk
        } else {
            self.graphics_pipeline = inner;
            StateCacherOutcome::NeedChange
        }
    }

    /// Checks whether we need to bind a compute pipeline. Returns `StateCacherOutcome::AlreadyOk`
    /// if the pipeline was already bound earlier, and `StateCacherOutcome::NeedChange` if you need
    /// to actually bind the pipeline.
    ///
    /// This function also updates the state cacher. The state cacher assumes that the state
    /// changes are going to be performed after this function returns.
    pub fn bind_compute_pipeline<P>(&mut self, pipeline: &P) -> StateCacherOutcome
        where P: ComputePipelineAbstract
    {
        let inner = pipeline.inner().internal_object();
        if inner == self.compute_pipeline {
            StateCacherOutcome::AlreadyOk
        } else {
            self.compute_pipeline = inner;
            StateCacherOutcome::NeedChange
        }
    }

    /// Checks whether we need to bind an index buffer. Returns `StateCacherOutcome::AlreadyOk`
    /// if the index buffer was already bound earlier, and `StateCacherOutcome::NeedChange` if you
    /// need to actually bind the buffer.
    ///
    /// This function also updates the state cacher. The state cacher assumes that the state
    /// changes are going to be performed after this function returns.
    pub fn bind_index_buffer<B>(&mut self, index_buffer: &B, ty: IndexType) -> StateCacherOutcome
        where B: ?Sized + BufferAccess
    {
        let value = {
            let inner = index_buffer.inner();
            (inner.buffer.internal_object(), inner.offset, ty)
        };

        if self.index_buffer == Some(value) {
            StateCacherOutcome::AlreadyOk
        } else {
            self.index_buffer = Some(value);
            StateCacherOutcome::NeedChange
        }
    }
}

/// Helper struct for comparing descriptor sets.
///
/// > **Note**: For safety reasons, if you drop/leak this struct before calling `compare` then the
/// > cache of the currently bound descriptor sets will be reset.
pub struct StateCacherDescriptorSets<'s> {
    // Reference to the parent's `poisonned_descriptor_sets`.
    poisonned: &'s mut bool,
    // Reference to the descriptor sets list to compare to.
    state: &'s mut SmallVec<[vk::DescriptorSet; 12]>,
    // Next offset within the list to compare to.
    offset: usize,
    // Contains the return value of `compare`.
    found_diff: Option<u32>,
}

impl<'s> StateCacherDescriptorSets<'s> {
    /// Adds a descriptor set to the list to compare.
    #[inline]
    pub fn add<S>(&mut self, set: &S)
        where S: ?Sized + DescriptorSet
    {
        let raw = set.inner().internal_object();

        if self.offset < self.state.len() {
            if self.state[self.offset] == raw {
                return;
            }

            self.state[self.offset] = raw;

        } else {
            self.state.push(raw);
        }

        if self.found_diff.is_none() {
            self.found_diff = Some(self.offset as u32);
        }
    }

    /// Compares your list to the list in cache, and returns the offset of the first set to bind.
    /// Returns `None` if the two lists were identical.
    ///
    /// After this function returns, the cache will be updated to match your list.
    #[inline]
    pub fn compare(self) -> Option<u32> {
        *self.poisonned = false;

        // Removing from the cache any set that wasn't added with `add`.
        if self.offset < self.state.len() {
            // TODO: SmallVec doesn't provide any method for this
            for _ in self.offset .. self.state.len() {
                self.state.remove(self.offset);
            }
        }

        self.found_diff
    }
}