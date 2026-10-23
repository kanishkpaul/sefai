//! GPTQ and AWQ int32 packings mapped exactly onto MLX affine groups.
//! Both formats dequantize as `scale * (q - zero)`; MLX computes `scale * q + bias`,
//! so each group's bias is `-scale * zero` and the packed integers are reused unchanged.

/// AutoAWQ GEMM stores output column `AWQ_ORDER[k]` of each eight in bits `4k..4k + 4`.
const AWQ_ORDER: [usize; 8] = [0, 2, 4, 6, 1, 3, 5, 7];

/// One linear layer's dimensions and grouping.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub inputs: usize,
    pub outputs: usize,
    pub bits: u32,
    pub group_size: usize,
}

/// One linear layer in MLX affine layout, row-major.
#[derive(Debug, PartialEq)]
pub(crate) struct Affine {
    /// `[outputs, inputs * bits / 32]`; consecutive inputs fill each word from the low bits up.
    pub weight: Vec<u32>,
    /// `[outputs, inputs / group_size]`.
    pub scales: Vec<f32>,
    /// `[outputs, inputs / group_size]`.
    pub biases: Vec<f32>,
}

impl Shape {
    fn per_word(self) -> usize {
        (32 / self.bits) as usize
    }
    fn groups(self) -> usize {
        self.inputs / self.group_size
    }
    fn mask(self) -> u32 {
        (1 << self.bits) - 1
    }
    fn check(self, qweight: usize, qzeros: usize, scales: usize) -> Result<(), String> {
        let per_word = self.per_word();
        if !matches!(self.bits, 4 | 8)
            || self.group_size == 0
            || self.inputs == 0
            || self.outputs == 0
            || !self.inputs.is_multiple_of(self.group_size)
            || !self.inputs.is_multiple_of(per_word)
            || !self.outputs.is_multiple_of(per_word)
        {
            return Err(format!(
                "layer {}x{} cannot hold {}-bit groups of {}",
                self.outputs, self.inputs, self.bits, self.group_size
            ));
        }
        let packed = self.inputs * self.outputs / per_word;
        let zeros = self.groups() * self.outputs / per_word;
        let grouped = self.groups() * self.outputs;
        if qweight != packed || qzeros != zeros || scales != grouped {
            return Err(format!(
                "packed tensors hold {qweight}, {qzeros} and {scales} values; the layer needs {packed}, {zeros} and {grouped}"
            ));
        }
        Ok(())
    }
}

/// Repacks GPTQ tensors: `qweight` is `[inputs / per_word, outputs]` packed along inputs,
/// `qzeros` is `[groups, outputs / per_word]` packed along outputs and `scales` is `[groups, outputs]`.
/// Version 1 checkpoints store each zero point minus one, so `zero_offset` adds it back.
pub(crate) fn gptq(
    qweight: &[u32],
    qzeros: &[u32],
    scales: &[f32],
    shape: Shape,
    zero_offset: bool,
) -> Result<Affine, String> {
    shape.check(qweight.len(), qzeros.len(), scales.len())?;
    let (per_word, rows) = (shape.per_word(), shape.inputs / shape.per_word());
    // Both layouts pack consecutive inputs from the low bits up, so a word transpose suffices.
    let mut weight = vec![0; qweight.len()];
    for row in 0..rows {
        for output in 0..shape.outputs {
            weight[output * rows + row] = qweight[row * shape.outputs + output];
        }
    }
    let offset = u32::from(zero_offset);
    let zero = |group: usize, output: usize| {
        let word = qzeros[group * (shape.outputs / per_word) + output / per_word];
        ((word >> (shape.bits as usize * (output % per_word))) & shape.mask()) + offset
    };
    let (scales, biases) = affine(scales, shape, zero);
    Ok(Affine {
        weight,
        scales,
        biases,
    })
}

/// Repacks AutoAWQ GEMM tensors: `qweight` is `[inputs, outputs / 8]` and `qzeros` is
/// `[groups, outputs / 8]`, both packed along outputs in `AWQ_ORDER`; `scales` is `[groups, outputs]`.
pub(crate) fn awq(
    qweight: &[u32],
    qzeros: &[u32],
    scales: &[f32],
    shape: Shape,
) -> Result<Affine, String> {
    if shape.bits != 4 {
        return Err("awq gemm packing requires 4 bits".into());
    }
    shape.check(qweight.len(), qzeros.len(), scales.len())?;
    let columns = shape.outputs / 8;
    let mut position = [0; 8];
    for (bits, column) in AWQ_ORDER.into_iter().enumerate() {
        position[column] = bits;
    }
    let value = |packed: &[u32], row: usize, output: usize| {
        (packed[row * columns + output / 8] >> (4 * position[output % 8])) & 0xf
    };
    let rows = shape.inputs / 8;
    let mut weight = vec![0; qweight.len()];
    for output in 0..shape.outputs {
        for row in 0..rows {
            weight[output * rows + row] = (0..8).fold(0, |word, k| {
                word | value(qweight, row * 8 + k, output) << (4 * k)
            });
        }
    }
    let (scales, biases) = affine(scales, shape, |group, output| value(qzeros, group, output));
    Ok(Affine {
        weight,
        scales,
        biases,
    })
}

/// Transposes `[groups, outputs]` scales and derives each group's bias from its zero point.
fn affine(
    scales: &[f32],
    shape: Shape,
    zero: impl Fn(usize, usize) -> u32,
) -> (Vec<f32>, Vec<f32>) {
    let groups = shape.groups();
    let mut transposed = vec![0.0; scales.len()];
    let mut biases = vec![0.0; scales.len()];
    for group in 0..groups {
        for output in 0..shape.outputs {
            let scale = scales[group * shape.outputs + output];
            transposed[output * groups + group] = scale;
            biases[output * groups + group] = -scale * zero(group, output) as f32;
        }
    }
    (transposed, biases)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic integers, zero points and scales for one small layer.
    struct Layer {
        shape: Shape,
        q: Vec<u32>,
        zeros: Vec<u32>,
        scales: Vec<f32>,
    }
    impl Layer {
        fn new(bits: u32) -> Self {
            let shape = Shape {
                inputs: 64,
                outputs: 16,
                bits,
                group_size: 32,
            };
            let mask = (1u32 << bits) - 1;
            let mut state = 0x9e37_79b9u32;
            let mut next = || {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state
            };
            let q = (0..shape.inputs * shape.outputs)
                .map(|_| next() & mask)
                .collect();
            let zeros = (0..shape.groups() * shape.outputs)
                .map(|_| next() & mask)
                .collect();
            let scales = (0..shape.groups() * shape.outputs)
                .map(|_| (next() % 1000) as f32 / 4096.0 + 0.001)
                .collect();
            Self {
                shape,
                q,
                zeros,
                scales,
            }
        }
        /// `q[input][output]`, `zeros[group][output]` and `scales[group][output]` dequantized.
        fn expected(&self, input: usize, output: usize) -> f32 {
            let index = (input / self.shape.group_size) * self.shape.outputs + output;
            self.scales[index]
                * (self.q[input * self.shape.outputs + output] as f32 - self.zeros[index] as f32)
        }
        fn check(&self, affine: &Affine) {
            let (shape, per_word) = (self.shape, self.shape.per_word());
            for output in 0..shape.outputs {
                for input in 0..shape.inputs {
                    let word = affine.weight[output * shape.inputs / per_word + input / per_word];
                    let q = (word >> (shape.bits as usize * (input % per_word))) & shape.mask();
                    let group = output * shape.groups() + input / shape.group_size;
                    let actual = affine.scales[group] * q as f32 + affine.biases[group];
                    let expected = self.expected(input, output);
                    assert!(
                        (actual - expected).abs() <= 1e-5 * expected.abs().max(1.0),
                        "output {output} input {input}: {actual} != {expected}"
                    );
                }
            }
        }
    }
    /// Packs `values[row][column]` along rows (GPTQ qweight) or columns (zeros), low bits first.
    fn pack(
        values: &[u32],
        rows: usize,
        columns: usize,
        bits: u32,
        along_rows: bool,
        order: Option<[usize; 8]>,
    ) -> Vec<u32> {
        let per_word = (32 / bits) as usize;
        let (packed_rows, packed_columns) = if along_rows {
            (rows / per_word, columns)
        } else {
            (rows, columns / per_word)
        };
        let mut packed = vec![0; packed_rows * packed_columns];
        for row in 0..packed_rows {
            for column in 0..packed_columns {
                packed[row * packed_columns + column] = (0..per_word).fold(0, |word, k| {
                    let (r, c) = if along_rows {
                        (row * per_word + k, column)
                    } else {
                        (row, column * per_word + order.map_or(k, |order| order[k]))
                    };
                    word | values[r * columns + c] << (bits as usize * k)
                });
            }
        }
        packed
    }

    #[test]
    fn gptq_repack_preserves_every_dequantized_weight() {
        for bits in [4, 8] {
            for zero_offset in [true, false] {
                let mut layer = Layer::new(bits);
                let shape = layer.shape;
                let stored = layer.zeros.clone();
                for zero in &mut layer.zeros {
                    *zero += u32::from(zero_offset);
                }
                let affine = gptq(
                    &pack(&layer.q, shape.inputs, shape.outputs, bits, true, None),
                    &pack(&stored, shape.groups(), shape.outputs, bits, false, None),
                    &layer.scales,
                    shape,
                    zero_offset,
                )
                .expect("repack");
                layer.check(&affine);
            }
        }
    }
    #[test]
    fn awq_repack_preserves_every_dequantized_weight() {
        let layer = Layer::new(4);
        let shape = layer.shape;
        let affine = awq(
            &pack(
                &layer.q,
                shape.inputs,
                shape.outputs,
                4,
                false,
                Some(AWQ_ORDER),
            ),
            &pack(
                &layer.zeros,
                shape.groups(),
                shape.outputs,
                4,
                false,
                Some(AWQ_ORDER),
            ),
            &layer.scales,
            shape,
        )
        .expect("repack");
        layer.check(&affine);
    }
    #[test]
    fn mismatched_tensor_sizes_are_rejected() {
        let layer = Layer::new(4);
        let shape = layer.shape;
        let qweight = pack(&layer.q, shape.inputs, shape.outputs, 4, true, None);
        let qzeros = pack(&layer.zeros, shape.groups(), shape.outputs, 4, false, None);
        assert!(gptq(&qweight[1..], &qzeros, &layer.scales, shape, true).is_err());
        assert!(gptq(&qweight, &qzeros, &layer.scales[1..], shape, true).is_err());
        let ungrouped = Shape {
            group_size: 48,
            ..shape
        };
        assert!(gptq(&qweight, &qzeros, &layer.scales, ungrouped, true).is_err());
        assert!(awq(&qweight, &qzeros, &layer.scales, Shape { bits: 8, ..shape }).is_err());
    }
}
