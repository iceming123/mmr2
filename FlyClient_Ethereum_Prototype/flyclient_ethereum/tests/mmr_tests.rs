use ethereum_types::H256;
use fly_eth::{InMemoryMerkleTree, MerkleTree, Proof, ProofBlock};
use flyclient_ethereum as fly_eth;
use serde_json;

fn get_hash(hash: &str) -> H256 {
    serde_json::from_str(&format!("{:?}", hash)).unwrap()
}

#[test]
fn test0() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");
    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");

    let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(block0, 17179869184u128, None);
    mmr.append_leaf(block1, 17171480576u128);

    let block_numbers = vec![ProofBlock::new(1, Some(mmr.get_difficulty_relation(1)))];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    assert!(Proof::verify_proof(&mut proof, block_numbers).is_ok());
}

#[test]
fn test1() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");
    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");
    let block2 = get_hash("0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9");
    let block3 = get_hash("0x3d6122660cc824376f11ee842f83addc3525e2dd6756b9bcf0affa6aa88cf741");
    let block4 = get_hash("0x23adf5a3be0f5235b36941bcb29b62504278ec5b9cdfa277b992ba4a7a3cd3a2");
    let block5 = get_hash("0xf37c632d361e0a93f08ba29b1a2c708d9caa3ee19d1ee8d2a02612bffe49f0a9");
    let block6 = get_hash("0x1f1aed8e3694a067496c248e61879cda99b0709a1dfbacd0b693750df06b326e");
    let block7 = get_hash("0xe0c7c0b46e116b874354dce6f64b8581bd239186b03f30a978e3dc38656f723a");
    let block8 = get_hash("0x2ce94342df186bab4165c268c43ab982d360c9474f429fec5565adfc5d1f258b");

    let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(block0, 0, None);

    mmr.append_leaf(block1, 1);
    mmr.append_leaf(block2, 2);
    mmr.append_leaf(block3, 3);
    mmr.append_leaf(block4, 4);
    mmr.append_leaf(block5, 5);
    mmr.append_leaf(block6, 6);
    mmr.append_leaf(block7, 7);
    mmr.append_leaf(block8, 8);

    let block_numbers = vec![
        ProofBlock::new(0, Some(mmr.get_difficulty_relation(0))),
        ProofBlock::new(1, Some(mmr.get_difficulty_relation(1))),
        ProofBlock::new(2, Some(mmr.get_difficulty_relation(2))),
        ProofBlock::new(3, Some(mmr.get_difficulty_relation(3))),
        ProofBlock::new(4, Some(mmr.get_difficulty_relation(4))),
        ProofBlock::new(5, Some(mmr.get_difficulty_relation(5))),
        ProofBlock::new(8, Some(mmr.get_difficulty_relation(8))),
    ];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    assert!(Proof::verify_proof(&mut proof, block_numbers).is_ok());
}

#[test]
fn test2() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");
    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");
    let block2 = get_hash("0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9");
    let block3 = get_hash("0x3d6122660cc824376f11ee842f83addc3525e2dd6756b9bcf0affa6aa88cf741");
    let block4 = get_hash("0x23adf5a3be0f5235b36941bcb29b62504278ec5b9cdfa277b992ba4a7a3cd3a2");
    let block5 = get_hash("0xf37c632d361e0a93f08ba29b1a2c708d9caa3ee19d1ee8d2a02612bffe49f0a9");
    let block6 = get_hash("0x1f1aed8e3694a067496c248e61879cda99b0709a1dfbacd0b693750df06b326e");
    let block7 = get_hash("0xe0c7c0b46e116b874354dce6f64b8581bd239186b03f30a978e3dc38656f723a");
    let block8 = get_hash("0x2ce94342df186bab4165c268c43ab982d360c9474f429fec5565adfc5d1f258b");
    let block9 = get_hash("0x997e47bf4cac509c627753c06385ac866641ec6f883734ff7944411000dc576e");

    let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(block0, 0, None);

    mmr.append_leaf(block1, 1);
    mmr.append_leaf(block2, 2);
    mmr.append_leaf(block3, 3);
    mmr.append_leaf(block3, 4);
    mmr.append_leaf(block3, 5);
    mmr.append_leaf(block3, 6);
    mmr.append_leaf(block3, 7);
    mmr.append_leaf(block4, 8);
    mmr.append_leaf(block5, 9);
    mmr.append_leaf(block6, 10);
    mmr.append_leaf(block4, 11);
    mmr.append_leaf(block5, 12);
    mmr.append_leaf(block5, 13);
    mmr.append_leaf(block5, 14);
    mmr.append_leaf(block7, 15);
    mmr.append_leaf(block8, 16);
    mmr.append_leaf(block0, 17);
    mmr.append_leaf(block3, 18);
    mmr.append_leaf(block5, 19);
    mmr.append_leaf(block5, 20);
    mmr.append_leaf(block5, 21);
    mmr.append_leaf(block5, 22);
    mmr.append_leaf(block0, 23);
    mmr.append_leaf(block5, 24);
    mmr.append_leaf(block5, 25);
    mmr.append_leaf(block9, 26);
    mmr.append_leaf(block7, 27);

    let block_numbers = vec![
        ProofBlock::new(1, Some(mmr.get_difficulty_relation(1))),
        ProofBlock::new(3, Some(mmr.get_difficulty_relation(3))),
        ProofBlock::new(5, Some(mmr.get_difficulty_relation(5))),
        ProofBlock::new(8, Some(mmr.get_difficulty_relation(8))),
        ProofBlock::new(16, Some(mmr.get_difficulty_relation(16))),
        ProofBlock::new(20, Some(mmr.get_difficulty_relation(20))),
    ];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    let result = Proof::verify_proof(&mut proof, block_numbers);

    assert!(result.is_ok());
}

#[test]
fn test3() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");
    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");
    let block2 = get_hash("0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9");
    let block3 = get_hash("0x3d6122660cc824376f11ee842f83addc3525e2dd6756b9bcf0affa6aa88cf741");
    let block4 = get_hash("0x23adf5a3be0f5235b36941bcb29b62504278ec5b9cdfa277b992ba4a7a3cd3a2");
    let block5 = get_hash("0xf37c632d361e0a93f08ba29b1a2c708d9caa3ee19d1ee8d2a02612bffe49f0a9");
    let block6 = get_hash("0x1f1aed8e3694a067496c248e61879cda99b0709a1dfbacd0b693750df06b326e");
    let block7 = get_hash("0xe0c7c0b46e116b874354dce6f64b8581bd239186b03f30a978e3dc38656f723a");
    let block8 = get_hash("0x2ce94342df186bab4165c268c43ab982d360c9474f429fec5565adfc5d1f258b");
    let block9 = get_hash("0x997e47bf4cac509c627753c06385ac866641ec6f883734ff7944411000dc576e");

    let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(block0, 0, None);

    mmr.append_leaf(block1, 1);
    mmr.append_leaf(block2, 2);
    mmr.append_leaf(block3, 3);
    mmr.append_leaf(block3, 4);
    mmr.append_leaf(block3, 5);
    mmr.append_leaf(block3, 6);
    mmr.append_leaf(block3, 7);
    mmr.append_leaf(block4, 8);
    mmr.append_leaf(block5, 9);
    mmr.append_leaf(block6, 10);
    mmr.append_leaf(block4, 11);
    mmr.append_leaf(block5, 12);
    mmr.append_leaf(block5, 13);
    mmr.append_leaf(block5, 14);
    mmr.append_leaf(block7, 15);
    mmr.append_leaf(block8, 16);
    mmr.append_leaf(block0, 17);
    mmr.append_leaf(block3, 18);
    mmr.append_leaf(block5, 19);
    mmr.append_leaf(block5, 20);
    mmr.append_leaf(block5, 21);
    mmr.append_leaf(block5, 22);
    mmr.append_leaf(block0, 23);
    mmr.append_leaf(block5, 24);
    mmr.append_leaf(block5, 25);
    mmr.append_leaf(block9, 26);
    mmr.append_leaf(block7, 27);

    let block_numbers = vec![ProofBlock::new(15, Some(mmr.get_difficulty_relation(15)))];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    let result = Proof::verify_proof(&mut proof, block_numbers);

    assert!(result.is_ok());
}

#[test]
fn test4() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");

    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");
    let block2 = get_hash("0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9");
    let block3 = get_hash("0x3d6122660cc824376f11ee842f83addc3525e2dd6756b9bcf0affa6aa88cf741");
    let block4 = get_hash("0x23adf5a3be0f5235b36941bcb29b62504278ec5b9cdfa277b992ba4a7a3cd3a2");
    let block5 = get_hash("0xf37c632d361e0a93f08ba29b1a2c708d9caa3ee19d1ee8d2a02612bffe49f0a9");
    let block6 = get_hash("0x1f1aed8e3694a067496c248e61879cda99b0709a1dfbacd0b693750df06b326e");
    let block7 = get_hash("0xe0c7c0b46e116b874354dce6f64b8581bd239186b03f30a978e3dc38656f723a");
    let block8 = get_hash("0x2ce94342df186bab4165c268c43ab982d360c9474f429fec5565adfc5d1f258b");

    let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(block0, 0, None);

    mmr.append_leaf(block1, 1);
    mmr.append_leaf(block2, 2);
    mmr.append_leaf(block3, 3);
    mmr.append_leaf(block4, 4);
    mmr.append_leaf(block5, 5);
    mmr.append_leaf(block6, 6);
    mmr.append_leaf(block7, 7);
    mmr.append_leaf(block8, 8);

    let block_numbers = vec![
        ProofBlock::new(1, Some(mmr.get_difficulty_relation(1))),
        ProofBlock::new(3, Some(mmr.get_difficulty_relation(3))),
        ProofBlock::new(4, Some(mmr.get_difficulty_relation(4))),
        ProofBlock::new(5, Some(mmr.get_difficulty_relation(5))),
        ProofBlock::new(8, Some(mmr.get_difficulty_relation(8))),
    ];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    assert!(Proof::verify_proof(&mut proof, block_numbers).is_ok());
}

#[test]
fn test5() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");
    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");
    let block2 = get_hash("0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9");
    let block3 = get_hash("0x3d6122660cc824376f11ee842f83addc3525e2dd6756b9bcf0affa6aa88cf741");
    let block4 = get_hash("0x23adf5a3be0f5235b36941bcb29b62504278ec5b9cdfa277b992ba4a7a3cd3a2");
    let block5 = get_hash("0xf37c632d361e0a93f08ba29b1a2c708d9caa3ee19d1ee8d2a02612bffe49f0a9");
    let block6 = get_hash("0x1f1aed8e3694a067496c248e61879cda99b0709a1dfbacd0b693750df06b326e");

    let mut mmr = MerkleTree::<InMemoryMerkleTree>::new(block0, 17179869184u128, None);

    mmr.append_leaf(block1, 17171480576u128);
    mmr.append_leaf(block2, 17163096064u128);
    mmr.append_leaf(block3, 17154715646u128);
    mmr.append_leaf(block4, 17146339321u128);
    mmr.append_leaf(block5, 17154711556u128);
    mmr.append_leaf(block6, 17146335232u128);

    let block_numbers = vec![
        ProofBlock::new(1, Some(mmr.get_difficulty_relation(1))),
        ProofBlock::new(2, Some(mmr.get_difficulty_relation(2))),
    ];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    assert!(Proof::verify_proof(&mut proof, block_numbers).is_ok());
}

#[test]
fn test6() {
    let block0 = get_hash("0xd4e56740f876aef8c010b86a40d5f56745a118d0906a34e69aec8c0db1cb8fa3");
    let block1 = get_hash("0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6");
    let block2 = get_hash("0xb495a1d7e6663152ae92708da4843337b958146015a2802f4193a410044698c9");
    let block3 = get_hash("0x3d6122660cc824376f11ee842f83addc3525e2dd6756b9bcf0affa6aa88cf741");
    let block4 = get_hash("0x23adf5a3be0f5235b36941bcb29b62504278ec5b9cdfa277b992ba4a7a3cd3a2");
    let block5 = get_hash("0xf37c632d361e0a93f08ba29b1a2c708d9caa3ee19d1ee8d2a02612bffe49f0a9");
    let block6 = get_hash("0x1f1aed8e3694a067496c248e61879cda99b0709a1dfbacd0b693750df06b326e");

    let mut mmr = fly_eth::MerkleTree::<InMemoryMerkleTree>::new(block0, 17179869184u128, None);

    mmr.append_leaf(block1, 17171480576u128);
    mmr.append_leaf(block2, 17163096064u128);
    mmr.append_leaf(block3, 17154715646u128);
    mmr.append_leaf(block4, 17146339321u128);
    mmr.append_leaf(block5, 17154711556u128);
    mmr.append_leaf(block6, 17146335232u128);

    let block_numbers = vec![
        ProofBlock::new(0, Some(mmr.get_difficulty_relation(0))),
        ProofBlock::new(1, Some(mmr.get_difficulty_relation(1))),
        ProofBlock::new(2, Some(mmr.get_difficulty_relation(2))),
        ProofBlock::new(1, Some(mmr.get_difficulty_relation(1))),
        ProofBlock::new(0, Some(mmr.get_difficulty_relation(0))),
    ];
    let mut block_numbers2 = block_numbers.iter().map(|block| block.number).collect();
    let mut proof = Proof::generate_proof(&mut mmr, &mut block_numbers2);

    assert!(Proof::verify_proof(&mut proof, block_numbers).is_ok());
}
