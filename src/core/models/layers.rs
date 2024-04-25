use crate::core::{
    graph::{Context, NodeIdentifier, Result, Shape},
    nn::prelude::initializers::Initializer,
};

impl Context {
    pub fn dense(
        &mut self,
        input_node: NodeIdentifier,
        out_size: u32,
        initializer: &Initializer,
        name: &str,
    ) -> Result<(NodeIdentifier, (NodeIdentifier, NodeIdentifier))> {
        let shape = self.nodes[input_node].shape.clone();
        let last_dim = shape.sizes[shape.ndims() - 1];
        let dtype = self.nodes[input_node].dtype;

        let weights_shape = Shape::from([last_dim, out_size]);
        let mut weights_name = name.to_owned();
        weights_name.push_str("_weights");
        let weights = self.parameter(weights_name, weights_shape, dtype)?;
        let weights_init = initializer.initialize(
            self,
            weights,
            self.nodes[input_node].shape.sizes[1] as usize,
        )?;

        let mut bias_shape = Shape::new();
        for _ in 0..(shape.ndims() - 1) {
            bias_shape.sizes.push(1u32);
        }
        bias_shape.sizes.push(out_size);
        let mut bias_name = name.to_owned();
        bias_name.push_str("_bias");
        let bias = self.parameter(bias_name, bias_shape, dtype)?;

        let matmul_node = self.matmul(input_node, weights_init)?;
        let dense_node = self.add(matmul_node, bias)?;

        Ok((dense_node, (weights_init, bias)))
    }
}