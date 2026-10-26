//! Decoding and rendering off the interface thread: a frame decoded once is kept on the GPU as its
//! preview, and a newer job replaces the one waiting in the render slot, so a drag never queues.

use crate::display::{self, Params};
use oni_graph::stages::Crop;
use oni_graph::{Graph, Image, Rect, Source, Stage};
use oni_pipeline::{CustomFlatField, Inputs, Pipeline, Request};
use oni_settings::frame::FlatFieldMask;
use oni_settings::id::Id;
use std::collections::HashMap;
use oni_settings::PipelineSettings;
use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};


mod decode;
mod door;
mod engine;
mod view;
mod previews;
mod worker;
mod histogram;
pub use door::graph;
pub(crate) use door::app_gpu;
pub(crate) use door::Door;
pub use {decode::*, engine::*, view::*, previews::*, worker::*, histogram::*};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod upload_tests;
#[cfg(test)]
mod door_tests;
#[cfg(test)]
mod kept_tests;
#[cfg(test)]
mod prewarm_tests;
#[cfg(test)]
mod inputs_tests;
#[cfg(test)]
mod tiles_tests;
