{ pkgs ? import <nixpkgs> {}, ... }:
with pkgs;
let llvmPackages = pkgs.llvmPackages;
    clang = llvmPackages.clang;
    libclang = llvmPackages.libclang;
    llvm = llvmPackages.llvm;
    stdenv = clang.stdenv;
in stdenv.mkDerivation {
    name = "c4dart";
    buildInputs = [
        pkgconfig
        llvm
        libclang
    ];
    LIBCLANG_PATH = "${libclang}/lib/libclang.so";
}
