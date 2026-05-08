pragma circom 2.1.8;

template FieldHash() {
    signal input x;
    signal output out;

    signal y;
    signal y2;
    signal y4;
    signal y5;

    y <== x + 5;
    y2 <== y * y;
    y4 <== y2 * y2;
    y5 <== y4 * y;
    out <== y5 + (x * 7) + 42;
}

template VrfMvp() {
    signal input sk;
    signal input alpha_hash;
    signal input beta;

    component h = FieldHash();
    component beta_hash = FieldHash();

    signal gamma;

    h.x <== alpha_hash;
    gamma <== sk * h.out;
    beta_hash.x <== gamma;
    beta === beta_hash.out;
}

component main { public [alpha_hash, beta] } = VrfMvp();
