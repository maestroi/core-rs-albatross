use algebra::mnt4_753::Fr as MNT4Fr;
use r1cs_core::SynthesisError;
use r1cs_std::bits::{boolean::Boolean, uint32::UInt32, uint8::UInt8};
use r1cs_std::mnt6_753::{G1Gadget, G2Gadget};

use crate::gadgets::mnt4::{PedersenCommitmentGadget, SerializeGadget};
use crate::utils::reverse_inner_byte_order;

/// This gadget is meant to calculate the "state commitment" in-circuit, which is simply a commitment,
/// for a given block, of the block number concatenated with the public_keys. We calculate it by first
/// serializing the block number and the public keys and feeding it to the Pedersen commitment
/// function, then we serialize the output and convert it to bytes. This provides an efficient way
/// of compressing the state and representing it across different curves.
pub struct StateCommitmentGadget;

impl StateCommitmentGadget {
    /// Calculates the state commitment.
    pub fn evaluate<CS: r1cs_core::ConstraintSystem<MNT4Fr>>(
        mut cs: CS,
        block_number: &UInt32,
        public_keys: &Vec<G2Gadget>,
        pedersen_generators: &Vec<G1Gadget>,
        sum_generator: &G1Gadget,
    ) -> Result<Vec<UInt8>, SynthesisError> {
        // Initialize Boolean vector.
        let mut bits: Vec<Boolean> = vec![];

        // The block number comes in little endian all the way.
        // So, a reverse will put it into big endian.
        let mut block_number_be = block_number.to_bits_le();
        block_number_be.reverse();
        bits.extend(block_number_be);

        // Convert each public key to bits and append it.
        for i in 0..public_keys.len() {
            bits.extend(SerializeGadget::serialize_g2(
                cs.ns(|| format!("serialize pks: {}", i)),
                &public_keys[i],
            )?);
        }

        // Calculate the Pedersen commitment.
        let pedersen_commitment = PedersenCommitmentGadget::evaluate(
            cs.ns(|| "pedersen commitment"),
            &bits,
            pedersen_generators,
            &sum_generator,
        )?;

        // Serialize the Pedersen commitment.
        let serialized_bits = SerializeGadget::serialize_g1(
            cs.ns(|| "serialize pedersen commitment"),
            &pedersen_commitment,
        )?;
        let serialized_bits = reverse_inner_byte_order(&serialized_bits[..]);

        // Convert to bytes.
        let mut bytes = Vec::new();
        for i in 0..serialized_bits.len() / 8 {
            bytes.push(UInt8::from_bits_le(&serialized_bits[i * 8..(i + 1) * 8]));
        }

        Ok(bytes)
    }
}