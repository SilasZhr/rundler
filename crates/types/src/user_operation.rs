// This file is part of Rundler.
//
// Rundler is free software: you can redistribute it and/or modify it under the
// terms of the GNU Lesser General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later version.
//
// Rundler is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
// without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with Rundler.
// If not, see https://www.gnu.org/licenses/.

use ethers::{
    abi::{encode, Token},
    types::{Address, Bytes, H256, U256},
    utils::keccak256,
};
use strum::IntoEnumIterator;

use crate::{
    entity::{Entity, EntityType},
    UserOperation,
};

/// Number of bytes in the fixed size portion of an ABI encoded user operation
const PACKED_USER_OPERATION_FIXED_LEN: usize = 480;

/// Unique identifier for a user operation from a given sender
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UserOperationId {
    sender: Address,
    nonce: U256,
}

impl UserOperation {
    /// Hash a user operation with the given entry point and chain ID.
    ///
    /// The hash is used to uniquely identify a user operation in the entry point.
    /// It does not include the signature field.
    pub fn op_hash(&self, entry_point: Address, chain_id: u64) -> H256 {
        keccak256(encode(&[
            Token::FixedBytes(keccak256(self.pack_for_hash()).to_vec()),
            Token::Address(entry_point),
            Token::Uint(chain_id.into()),
        ]))
        .into()
    }

    /// Get the unique identifier for this user operation from its sender
    pub fn id(&self) -> UserOperationId {
        UserOperationId {
            sender: self.sender,
            nonce: self.nonce,
        }
    }

    /// Get the address of the factory entity associated with this user operation, if any
    pub fn factory(&self) -> Option<Address> {
        Self::get_address_from_field(&self.init_code)
    }

    /// Get the address of the paymaster entity associated with this user operation, if any
    pub fn paymaster(&self) -> Option<Address> {
        Self::get_address_from_field(&self.paymaster_and_data)
    }

    /// Extracts an address from the beginning of a data field
    /// Useful to extract the paymaster address from paymaster_and_data
    /// and the factory address from init_code
    pub fn get_address_from_field(data: &Bytes) -> Option<Address> {
        if data.len() < 20 {
            None
        } else {
            Some(Address::from_slice(&data[..20]))
        }
    }

    /// Efficient calculation of the size of a packed user operation
    pub fn abi_encoded_size(&self) -> usize {
        PACKED_USER_OPERATION_FIXED_LEN
            + pad_len(&self.init_code)
            + pad_len(&self.call_data)
            + pad_len(&self.paymaster_and_data)
            + pad_len(&self.signature)
    }

    /// Compute the amount of heap memory the UserOperation takes up.
    pub fn heap_size(&self) -> usize {
        self.init_code.len()
            + self.call_data.len()
            + self.paymaster_and_data.len()
            + self.signature.len()
    }

    /// Gets the byte array representation of the user operation to be used in the signature
    pub fn pack_for_hash(&self) -> Bytes {
        let hash_init_code = keccak256(self.init_code.clone());
        let hash_call_data = keccak256(self.call_data.clone());
        let hash_paymaster_and_data = keccak256(self.paymaster_and_data.clone());

        encode(&[
            Token::Address(self.sender),
            Token::Uint(self.nonce),
            Token::FixedBytes(hash_init_code.to_vec()),
            Token::FixedBytes(hash_call_data.to_vec()),
            Token::Uint(self.call_gas_limit),
            Token::Uint(self.verification_gas_limit),
            Token::Uint(self.pre_verification_gas),
            Token::Uint(self.max_fee_per_gas),
            Token::Uint(self.max_priority_fee_per_gas),
            Token::FixedBytes(hash_paymaster_and_data.to_vec()),
        ])
        .into()
    }

    /// Gets an iterator on all entities associated with this user operation
    pub fn entities(&'_ self) -> impl Iterator<Item = Entity> + '_ {
        EntityType::iter().filter_map(|entity| {
            self.entity_address(entity)
                .map(|address| Entity::new(entity, address))
        })
    }

    /// Gets the address of the entity of the given type associated with this user operation, if any
    fn entity_address(&self, entity: EntityType) -> Option<Address> {
        match entity {
            EntityType::Account => Some(self.sender),
            EntityType::Paymaster => self.paymaster(),
            EntityType::Factory => self.factory(),
            EntityType::Aggregator => None,
        }
    }
}

/// Calculates the size a byte array padded to the next largest multiple of 32
fn pad_len(b: &Bytes) -> usize {
    (b.len() + 31) & !31
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ethers::{
        abi::AbiEncode,
        types::{Bytes, U256},
    };

    use super::*;

    #[test]
    fn test_hash_zeroed() {
        // Testing a user operation hash against the hash generated by the
        // entrypoint contract getUserOpHash() function with entrypoint address
        // at 0x66a15edcc3b50a663e72f1457ffd49b9ae284ddc and chain ID 1337.
        //
        // UserOperation = {
        //     sender: '0x0000000000000000000000000000000000000000',
        //     nonce: 0,
        //     initCode: '0x',
        //     callData: '0x',
        //     callGasLimit: 0,
        //     verificationGasLimit: 0,
        //     preVerificationGas: 0,
        //     maxFeePerGas: 0,
        //     maxPriorityFeePerGas: 0,
        //     paymasterAndData: '0x',
        //     signature: '0x',
        //   }
        //
        // Hash: 0xdca97c3b49558ab360659f6ead939773be8bf26631e61bb17045bb70dc983b2d
        let operation = UserOperation {
            sender: "0x0000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
            nonce: U256::zero(),
            init_code: Bytes::default(),
            call_data: Bytes::default(),
            call_gas_limit: U256::zero(),
            verification_gas_limit: U256::zero(),
            pre_verification_gas: U256::zero(),
            max_fee_per_gas: U256::zero(),
            max_priority_fee_per_gas: U256::zero(),
            paymaster_and_data: Bytes::default(),
            signature: Bytes::default(),
        };
        let entry_point = "0x66a15edcc3b50a663e72f1457ffd49b9ae284ddc"
            .parse()
            .unwrap();
        let chain_id = 1337;
        let hash = operation.op_hash(entry_point, chain_id);
        assert_eq!(
            hash,
            "0xdca97c3b49558ab360659f6ead939773be8bf26631e61bb17045bb70dc983b2d"
                .parse()
                .unwrap()
        );
    }

    #[test]
    fn test_hash() {
        // Testing a user operation hash against the hash generated by the
        // entrypoint contract getUserOpHash() function with entrypoint address
        // at 0x66a15edcc3b50a663e72f1457ffd49b9ae284ddc and chain ID 1337.
        //
        // UserOperation = {
        //     sender: '0x1306b01bc3e4ad202612d3843387e94737673f53',
        //     nonce: 8942,
        //     initCode: '0x6942069420694206942069420694206942069420',
        //     callData: '0x0000000000000000000000000000000000000000080085',
        //     callGasLimit: 10000,
        //     verificationGasLimit: 100000,
        //     preVerificationGas: 100,
        //     maxFeePerGas: 99999,
        //     maxPriorityFeePerGas: 9999999,
        //     paymasterAndData:
        //       '0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        //     signature:
        //       '0xda0929f527cded8d0a1eaf2e8861d7f7e2d8160b7b13942f99dd367df4473a',
        //   }
        //
        // Hash: 0x484add9e4d8c3172d11b5feb6a3cc712280e176d278027cfa02ee396eb28afa1
        let operation = UserOperation {
            sender: "0x1306b01bc3e4ad202612d3843387e94737673f53"
                .parse()
                .unwrap(),
            nonce: 8942.into(),
            init_code: "0x6942069420694206942069420694206942069420"
                .parse()
                .unwrap(),
            call_data: "0x0000000000000000000000000000000000000000080085"
                .parse()
                .unwrap(),
            call_gas_limit: 10000.into(),
            verification_gas_limit: 100000.into(),
            pre_verification_gas: 100.into(),
            max_fee_per_gas: 99999.into(),
            max_priority_fee_per_gas: 9999999.into(),
            paymaster_and_data:
                "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .parse()
                    .unwrap(),
            signature: "0xda0929f527cded8d0a1eaf2e8861d7f7e2d8160b7b13942f99dd367df4473a"
                .parse()
                .unwrap(),
        };
        let entry_point = "0x66a15edcc3b50a663e72f1457ffd49b9ae284ddc"
            .parse()
            .unwrap();
        let chain_id = 1337;
        let hash = operation.op_hash(entry_point, chain_id);
        assert_eq!(
            hash,
            "0x484add9e4d8c3172d11b5feb6a3cc712280e176d278027cfa02ee396eb28afa1"
                .parse()
                .unwrap()
        );
    }

    #[test]
    fn test_get_address_from_field() {
        let paymaster_and_data: Bytes =
            "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .parse()
                .unwrap();
        let address = UserOperation::get_address_from_field(&paymaster_and_data).unwrap();
        assert_eq!(
            address,
            "0x0123456789abcdef0123456789abcdef01234567"
                .parse()
                .unwrap()
        );
    }

    #[test]
    fn test_abi_encoded_size() {
        let user_operation = UserOperation {
            sender: "0xe29a7223a7e040d70b5cd460ef2f4ac6a6ab304d"
                .parse()
                .unwrap(),
            nonce: U256::from_dec_str("3937668929043450082210854285941660524781292117276598730779").unwrap(),
            init_code: Bytes::default(),
            call_data: Bytes::from_str("0x5194544700000000000000000000000058440a3e78b190e5bd07905a08a60e30bb78cb5b000000000000000000000000000000000000000000000000000009184e72a000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap(),
            call_gas_limit: 40_960.into(),
            verification_gas_limit: 75_099.into(),
            pre_verification_gas: 46_330.into(),
            max_fee_per_gas: 105_000_000.into(),
            max_priority_fee_per_gas: 105_000_000.into(),
            paymaster_and_data: Bytes::from_str("0xc03aac639bb21233e0139381970328db8bceeb6700006508996f000065089a9b0000000000000000000000000000000000000000ca7517be4e51ca2cde69bc44c4c3ce00ff7f501ce4ee1b3c6b2a742f579247292e4f9a672522b15abee8eaaf1e1487b8e3121d61d42ba07a47f5ccc927aa7eb61b").unwrap(),
            signature: Bytes::from_str("0x00000000f8a0655423f2dfbb104e0ff906b7b4c64cfc12db0ac5ef0fb1944076650ce92a1a736518e5b6cd46c6ff6ece7041f2dae199fb4c8e7531704fbd629490b712dc1b").unwrap(),
        };

        assert_eq!(
            user_operation.clone().encode().len(),
            user_operation.abi_encoded_size()
        );
    }
}
