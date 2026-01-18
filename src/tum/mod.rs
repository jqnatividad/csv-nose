//! Table Uniformity Method (TUM) for CSV dialect detection.
//!
//! This module implements the algorithm from:
//! "Wrangling Messy CSV Files by Detecting Row and Type Patterns"
//! by van den Burg, Nazábal, and Sutton (2019).

pub mod potential_dialects;
pub mod regexes;
pub mod score;
pub mod table;
pub mod type_detection;
pub mod uniformity;
