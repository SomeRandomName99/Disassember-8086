# What is this project?

An 8086 disassembler written as a Rust learning project.

## Testing the Disassembler

To test the output of the disassembler:

1. Use NASM to assemble `test.asm`
2. Run the disassembler on the binary and pipe the output to a file:
   ```bash
   cargo run src/test > output_test.asm
   ```
3. use NASM on `output_test.asm`
4. Use a diff tool to make sure that both binaries are equal!

