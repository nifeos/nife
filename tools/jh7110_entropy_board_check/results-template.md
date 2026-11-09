# jh7110_entropy board check: results

| id | measure | value | criterion | verdict |
|---|---|---|---|---|
| R1 | crate commit the payload was built against | _ | 40 hex digits | _ |
| R2 | model (1 means the QEMU rehearsal) | _ | 0 on the board | _ |
| R3 | how the payload was loaded: the boot script's `payload came from`, or typed at the prompt | _ | net, or prompt | _ |
| R4 | how the board came up before this run (cold power-on, or what) | _ | recorded | _ |
| R5 | timebase in hertz, and whether the device tree gave it | _ | from_tree=1 | _ |
| R6 | TRNG window, compatible, and whether the tree named it | _ | 0x1600c000, from_tree=1 | _ |
| R7 | clocks enabled and reset released | _ | clocks=1 released=1 | _ |
| R8 | the run reached its end record | _ | 1 | _ |
| R9 | records dropped for a bad checksum | _ | recorded | _ |
| 1a | first reseed completed by polling, and STAT.SEEDED after it | _ | done=1 seeded=1 | _ |
| 1b | first generation's outcome | _ | ready | _ |
| 1c | second generation's outcome | _ | ready | _ |
| 1d | first generation's 32 bytes | _ | not all zero | _ |
| 1e | second generation's 32 bytes | _ | not all zero, and differ from 1d | _ |
| 2a | STAT.SEEDED as the board leaves reset | _ | recorded | _ |
| 2b | STAT.LAST_RESEED as the board leaves reset | _ | recorded (7 is unseeded) | _ |
| 2c | STAT.LAST_RESEED after the first reseed | _ | recorded | _ |
| 3a | STAT.R256 before the mode write | _ | recorded | _ |
| 3b | MODE before the mode write | _ | recorded | _ |
| 3c | FEATURES | _ | recorded | _ |
| 3d | BCONF | _ | recorded | _ |
| 3e | STAT.R256 after MODE_256_BIT_OUTPUT is written | _ | 1 | _ |
| 3f | which value of BCONF bit 3 means 256 bits | _ | derived from 3a and 3d | _ |
| 3g | FEATURES bits 1:0 and BCONF bit 2, the widest output each says the block was built for | _ | max_rand_length=1 max_prng_len=1 (both 256-bit), as read in 3c and 3d | _ |
| 4a | RAND_RDY with RAND_GENERATING, RAND_RDY acknowledged first (1000 trials) | _ | timeouts=0; together recorded | _ |
| 4b | RAND_RDY with RAND_GENERATING, second GENERATE over a latched RAND_RDY | _ | timeouts=0; together recorded | _ |
| 5a | STAT bits outside every field the crate names, ever seen | _ | recorded | _ |
| 5b | LAST_RESEED values seen (bit n set means value n was seen) | _ | recorded | _ |
| 5c | STAT bits 19 to 23 (above a 3-bit LAST_RESEED) ever set | _ | recorded | _ |
| 6a | generate to RAND_RDY, ticks | _ | timeouts=0 | _ |
| 6b | generate to RAND_RDY, microseconds (median, maximum) | _ | recorded | _ |
| 6c | reseed to SEED_DONE, ticks | _ | timeouts=0 | _ |
| 6d | reseed to SEED_DONE, microseconds (median, maximum) | _ | recorded | _ |
| 6e | one read of the time CSR, minimum ticks | _ | recorded | _ |
| 7a | ISTAT after the bring-up clear | _ | no documented bit left | _ |
| 7b | ISTAT bit 3 with AUTO_RQSTS=1 | _ | recorded; residue has no documented bit | _ |
| 7c | ISTAT bit 2 with AUTO_AGE=1 | _ | recorded; residue has no documented bit | _ |
| 8a | long run of driver-path generations | _ | all_zero=0 wrong_output_width=0 timeouts=0 | _ |
| 8b | first LFSR_LOCKUP: does a reseed clear it, and does the next generation serve | _ | next_outcome=ready, or none seen | _ |
| 9a | capture: bytes whose records verified | _ | bytes>=1048576 | _ |
| 9b | FIPS 140-2 blocks failed | _ | at most 3 | _ |
| 9c | Shannon entropy, bits per byte | _ | at least 7.99 | _ |
| 9d | chi-square over byte values, upper-tail p | _ | between 0.001 and 0.999 | _ |
| 9e | mean byte and lag-1 serial correlation | _ | mean within 0.5 of 127.5, abs(r) < 0.005 | _ |
| 9f | repeated 32-byte generations in the capture | _ | 0 | _ |
| 9g | outside statistical suite, if one was run (name and summary) | _ | recorded | _ |
