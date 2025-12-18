# Process manager

This repo is a learning project to get familiar with Rust.
Any similarities to existing projects are purely intentional.

## Features
* You can start multiple Laravel Queue workers in parallel
* You can monitor the output of each worker

## Development
You need to have Rust installed. You can get it from [here](https://www.rust-lang.org/tools/install).

1. Clone the repository:
   ```bash
   git clone git@github.com:Katalam/process-manager.git
   ```
2. Navigate to the project directory:
   ```bash
    cd process-manager
   ```
3. Build the project:
   ```bash
   cargo build
   ```
4. Run the project:
   ```bash
    cargo run --package process-manager --binprocess-manager 
    ```
