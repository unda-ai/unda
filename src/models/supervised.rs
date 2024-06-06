use xla::{Literal, PjRtBuffer, PjRtDevice, PjRtLoadedExecutable};

use crate::graph::{Context, ContextError, Node, NodeIdentifier, Result};

pub struct SupervisedModel<
    const P: usize,
    const I: usize,
    const O: usize,
    const T: usize,
    const M: usize,
> {
    // forward computation of the network without loss
    pub(crate) network: Context,
    // wraps the node identifiers for the parameters of the network
    // will be buffers at execution
    pub(crate) params: [NodeIdentifier; P],
    // list of input nodes
    // will be literals not buffers at executation
    pub(crate) inputs: [NodeIdentifier; I],
    // list of output nodes
    // will be buffers at execution
    pub(crate) outputs: [NodeIdentifier; O],

    // separate context which takes parameters, outputs, and targets
    pub(crate) compute_metrics: Context,
    // additional inputs to compute_metrics as the targets of the supervised learning algorithm
    pub(crate) targets: [NodeIdentifier; T],
    // index into compute_metrics context to find differentiable loss function
    pub(crate) loss: NodeIdentifier,
    // points to additional metrics like accuracy
    pub(crate) auxiliary_metrics: [NodeIdentifier; M],

    // executes the network context without Evaluationuating metrics
    pub(crate) inference_computation: xla::XlaComputation,
    // executes the network and gradient metrics
    pub(crate) evaluation_computation: xla::XlaComputation,
    // executes the network and gradient metrics and returns derivatives of the parameters
    pub(crate) gradient_computation: xla::XlaComputation,
}

impl<const P: usize, const I: usize, const O: usize, const T: usize, const M: usize>
    SupervisedModel<P, I, O, T, M>
{
    // this function should
    // build the inference_computation from the network context
    // fuse the network and compute_metrics contexts and build the evaluation_computation
    // further augment the context to return derivatives of all params and then build the gradient_computation
    pub fn new(
        mut network: Context,
        params: [NodeIdentifier; P],
        inputs: [NodeIdentifier; I],
        outputs: [NodeIdentifier; O],
        compute_metrics: Context,
        targets: [NodeIdentifier; T],
        loss: NodeIdentifier,
        auxiliary_metrics: [NodeIdentifier; M],
    ) -> Result<Self> {
        let inference_computation = network.build("inference_computation", outputs)?;
        let mut eval_context = network.clone();


        //Fuse compute_metrics to the end of eval_context
        //compute_metrics will take in outputs and targets as inputs
        //outputs is a direct output of inference context
        //targets are supplied in constructor

        //TODO
        

        let evaluation_computation = eval_context.build("evaluation_computation", [loss])?;
        let mut grad_context = eval_context.clone();

        //Gradient computation: diff loss of eval_context wrt all params
        let mut grads = [NodeIdentifier::default(); P];
        for i in 0..P {
            grads[i] = grad_context.diff(loss, params[i])?;
        }

        let gradient_computation = grad_context.build("gradient_computation", grads)?;

        Ok(Self { 
            network,
            params,
            inputs,
            outputs,
            compute_metrics,
            targets,
            loss,
            auxiliary_metrics,
            inference_computation,
            evaluation_computation,
            gradient_computation
        })
    }

    pub fn compile_inference(
        &self,
        client: xla::PjRtClient,
    ) -> Result<SupervisedInferenceExecutable<P, I, O>> {
        let loaded_inf_exec = self.inference_computation.compile(&client)?;

        let supervised_inf = SupervisedInferenceExecutable::from_executable(loaded_inf_exec);

        Ok(supervised_inf)
    }
    pub fn compile_evaluation(
        &self,
        client: xla::PjRtClient,
    ) -> Result<SupervisedEvaluationExecutable<P, I, O, T, M>> {
        let loaded_eval_exec = self.evaluation_computation.compile(&client)?;

        let supervised_eval = SupervisedEvaluationExecutable::from_executable(loaded_eval_exec);

        Ok(supervised_eval)
    }
    pub fn compile_gradient(
        &self,
        client: xla::PjRtClient,
    ) -> Result<SupervisedGradientExecutable<P, I, O, T, M>> {
        let loaded_grad_exec = self.gradient_computation.compile(&client)?;

        let supervised_grad = SupervisedGradientExecutable::from_executable(loaded_grad_exec);

        Ok(supervised_grad)
    }
}

pub struct SupervisedInferenceExecutable<const P: usize, const I: usize, const O: usize> {
    pub(crate) executable: xla::PjRtLoadedExecutable,
}

impl<const P: usize, const I: usize, const O: usize> SupervisedInferenceExecutable<P, I, O> {
    pub fn run(
        &self,
        parameters: [PjRtBuffer; P],
        inputs: [Literal; I],
    ) -> Result<
        // network outputs
        [PjRtBuffer; O]
        > {
        let mut input_buff = vec![];

        //Probably some better way to get the device than just first index
        //Potentially could cross against parameters client devices and seeing matches
        //Using None for now as it will use the default device.. I don't see a problem with that
        //let device = self.executable.client().devices()[0];

        for input in inputs {
            input_buff.push(self.executable.client().buffer_from_host_literal(None, &input)?);
        }

        input_buff.extend(parameters.into_iter());

        let res: std::result::Result<[PjRtBuffer; O], _> = self.executable.execute_b(&input_buff)?
            .into_iter()
            .flatten()
            .collect::<Vec<PjRtBuffer>>()
            .try_into();

        match res {
            Ok(out_slice) => Ok(out_slice),
            Err(_) => Err(ContextError::IncorrectOutputSizeError(O, input_buff.len()))
        }
    }

    pub fn from_executable(executable: xla::PjRtLoadedExecutable) -> Self {
        SupervisedInferenceExecutable { executable }
    }
}

pub struct SupervisedEvaluationExecutable<
    const P: usize,
    const I: usize,
    const O: usize,
    const T: usize,
    const M: usize,
> {
    pub(crate) executable: xla::PjRtLoadedExecutable,
}

impl<const P: usize, const I: usize, const O: usize, const T: usize, const M: usize>
    SupervisedEvaluationExecutable<P, I, O, T, M>
{
    pub fn run(
        &self,
        parameters: [PjRtBuffer; P],
        inputs: [Literal; I],
        targets: [Literal; T],
    ) -> Result<(
        // network outputs
        [PjRtBuffer; O],
        // loss
        PjRtBuffer,
        // auxiliary metrics
        [PjRtBuffer; M],
    )> {
        todo!()
    }
    pub fn from_executable(executable: xla::PjRtLoadedExecutable) -> Self {
        SupervisedEvaluationExecutable { executable }
    }

}

pub struct SupervisedGradientExecutable<
    const P: usize,
    const I: usize,
    const O: usize,
    const T: usize,
    const M: usize,
> {
    pub(crate) executable: xla::PjRtLoadedExecutable,
}

impl<const P: usize, const I: usize, const O: usize, const T: usize, const M: usize>
    SupervisedGradientExecutable<P, I, O, T, M>
{
    pub fn run(
        &self,
        parameters: [PjRtBuffer; P],
        inputs: [Literal; I],
        targets: [Literal; T],
    ) -> Result<(
        // network outputs
        [PjRtBuffer; O],
        // loss
        PjRtBuffer,
        // auxiliary metrics
        [PjRtBuffer; M],
        // gradients
        [PjRtBuffer; P],
    )> {
        todo!()
    }
    pub fn from_executable(executable: xla::PjRtLoadedExecutable) -> Self {
        SupervisedGradientExecutable { executable }
    }
}
