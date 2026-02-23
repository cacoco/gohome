{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/binary-caching/
  cachix.enable = true;
  cachix.pull = [ "pre-commit-hooks" ];
  
  # https://devenv.sh/basics/
  env.RUSTC_ICE = "0";

  # https://devenv.sh/packages/
  packages = [
    pkgs.actionlint
    pkgs.cargo
    pkgs.cargo-edit
    pkgs.cargo-nextest
    pkgs.cargo-watch
    pkgs.docker
    pkgs.git
    pkgs.go-task
    pkgs.jq
    pkgs.markdownlint-cli
    pkgs.toml-cli
  ];

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "stable";
    components = [ "rustc" "cargo" "clippy" "rustfmt" "rust-analyzer" ];
    targets = ["x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl"];
  };

  # https://devenv.sh/processes/
  # processes.cargo-watch.exec = "cargo-watch";

  # https://devenv.sh/services/
  # services.postgres.enable = true;

  # https://devenv.sh/scripts/
  # scripts.hello.exec = ''
  # '';

  # enterShell = ''
  # '';

  # https://devenv.sh/tasks/
  # tasks = {
  #   "myproj:setup".exec = "mytool build";
  #   "devenv:enterShell".after = [ "myproj:setup" ];
  # };

  # https://devenv.sh/tests/
  enterTest = ''
    echo "Running tests"
    git --version | grep --color=auto "${pkgs.git.version}"
  '';

  # https://devenv.sh/git-hooks/
  # git-hooks.hooks.shellcheck.enable = true;
  git-hooks.hooks = {
    actionlint = {
      enable = true;
      entry = "actionlint";
    };
    clippy = {
      enable = true;
      entry = "cargo clippy --all-targets --all-features --fix --allow-staged";
      args = ["--" "-D warnings"];
    };
    cuefmt = {
      enable = true;
      entry = "cue fmt";
      args = ["--verbose" "--simplify" "--files"];
      files = "^cue/.*\.cue$";
      pass_filenames = true;
    };
    cuevet = {
      enable = true;
      entry = "cue vet";
      args = ["--concrete"];
      files = "^cue/.*\.cue$";
      pass_filenames = true;
    };
    markdownlint = {
      enable = true;
      entry = "markdownlint";
      args = ["**/*.md" "--fix"];
    };
    rustfmt.enable = true;
    shellcheck.enable = true;
    shfmt.enable = true;
  };

  # See full reference at https://devenv.sh/reference/options/
}
