//! A recipe's key: what the caches downstream of a render hang on, so it names everything it reads.

use oni_graph::stages::{BoxBlur3, Gain};
use oni_graph::{Graph, Image};

#[test]
fn a_recipe_s_key_names_its_sources_stages_parameters_and_auxiliary_images() {
    let Ok(mut g) = Graph::new().map_err(|e| println!("SKIPPED a_recipe_s_key: {e}")) else { return };
    let pixels = vec![[0.5f32, 0.4, 0.3, 1.0]; 16 * 8];
    let (s, aux, other) = (g.upload(16, 8, &pixels).unwrap(), g.upload(16, 8, &pixels).unwrap(), g.upload(16, 8, &pixels).unwrap());
    let recipe = |gain: f32, aux: &oni_graph::Source| Image::new(&s).then(Gain([gain; 4])).then_reading(BoxBlur3, &[Image::new(aux)]);
    assert_eq!(recipe(1.2, &aux).key(), recipe(1.2, &aux).key(), "two recordings of one recipe");
    // Twins: one parameter, one auxiliary image or the source swapped each name another recipe.
    assert_ne!(recipe(1.3, &aux).key(), recipe(1.2, &aux).key());
    assert_ne!(recipe(1.2, &other).key(), recipe(1.2, &aux).key());
    assert_ne!(Image::new(&other).then(Gain([1.2; 4])).key(), Image::new(&s).then(Gain([1.2; 4])).key());
}

#[test]
fn a_recipe_evaluated_again_keys_alike_where_it_is_read() {
    let Ok(mut g) = Graph::new().map_err(|e| println!("SKIPPED a_recipe_evaluated_again: {e}")) else { return };
    let pixels = vec![[0.5f32, 0.4, 0.3, 1.0]; 16 * 8];
    let s = g.upload(16, 8, &pixels).unwrap();
    let recipe = Image::new(&s).then(Gain([1.2; 4]));
    let (first, again) = (g.materialise(&recipe).unwrap(), g.materialise(&recipe).unwrap());
    let read = |plate: &oni_graph::Source| Image::new(&s).then_reading(BoxBlur3, &[Image::new(plate)]).key();
    assert_eq!(read(&first), read(&again), "what reads a recipe made again keys as what read it the first time");
    assert_ne!(first.id(), again.id(), "each evaluation its own texture, which a canvas takes once");
    // Twins: another parameter, its half-float evaluation or the same pixels uploaded each key apart.
    assert_ne!(read(&g.materialise(&Image::new(&s).then(Gain([1.3; 4]))).unwrap()), read(&first));
    assert_ne!(read(&g.materialise_half(&recipe).unwrap()), read(&first));
    let rendered = g.render(&recipe, recipe.extent(), None).unwrap();
    let uploaded = g.upload(16, 8, &rendered).unwrap();
    assert_ne!(read(&uploaded), read(&first));
}
