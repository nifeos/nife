//! The planner's tests, in three files by what they exercise. A test reaches the crate's own
//! `plan_against`, and a plan against nife's program table with an expansion, as `super::plan_against`
//! and `super::plan`, because the helpers of the same name in `test_support` shadow them.

use crate::plan_against;
use crate::test_support::plan_named as plan;

mod designation;
mod operands;
mod streams;
