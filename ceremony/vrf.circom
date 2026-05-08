pragma circom 2.1.8;

include "circomlib/circuits/poseidon.circom";

template VrfMvp() {
    signal input sk;
    signal input alpha_hash;
    signal input beta;

    component h = Poseidon(1);
    component beta_hash = Poseidon(1);

    signal gamma;

    h.inputs[0] <== alpha_hash;
    gamma <== sk * h.out;
    beta_hash.inputs[0] <== gamma;
    beta === beta_hash.out;
}

component main { public [alpha_hash, beta] } = VrfMvp();
