# jh7110_entropy board check: results

| id | measure | value | criterion | verdict |
|---|---|---|---|---|
| R1 | crate commit the payload was built against | ee0bde8feb9f78a67da011c928f9a2a697f3bbc1 | 40 hex digits | PASS |
| R2 | model (1 means the QEMU rehearsal) | 0 | 0 on the board | PASS |
| R3 | how the payload was loaded: the boot script's `payload came from`, or typed at the prompt | net | net, or prompt | PASS |
| R4 | how the board came up before this run (cold power-on, or what) | cold power-on by the person | recorded | RECORDED |
| R5 | timebase in hertz, and whether the device tree gave it | 4000000 from_tree=1 | from_tree=1 | PASS |
| R6 | TRNG window, compatible, and whether the tree named it | 0x1600c000 starfive,trng from_tree=1 | 0x1600c000, from_tree=1 | PASS |
| R7 | clocks enabled and reset released | clocks=1 released=1 | clocks=1 released=1 | PASS |
| R8 | the run reached its end record | 1 | 1 | PASS |
| R9 | records dropped for a bad checksum | 0 | recorded | RECORDED |
| 1a | first reseed completed by polling, and STAT.SEEDED after it | done=1 seeded=1 | done=1 seeded=1 | PASS |
| 1b | first generation's outcome | ready | ready | PASS |
| 1c | second generation's outcome | ready | ready | PASS |
| 1d | first generation's 32 bytes | ef18cb44db7c60acab169124bd8e9ff92faf69c5ec19e75c65048652e2b05f81 | not all zero | PASS |
| 1e | second generation's 32 bytes | 4e363e3b5a889b13a1c4b5aab2a9e06bbe5661a83b0560197ed8bba2fb25266b | not all zero, and differ from 1d | PASS |
| 2a | STAT.SEEDED as the board leaves reset | 1 | recorded | RECORDED |
| 2b | STAT.LAST_RESEED as the board leaves reset | 4 | recorded (7 is unseeded) | RECORDED |
| 2c | STAT.LAST_RESEED after the first reseed | 0 | recorded | RECORDED |
| 3a | STAT.R256 before the mode write | 1 | recorded | RECORDED |
| 3b | MODE before the mode write | 0x00000008 | recorded | RECORDED |
| 3c | FEATURES | 0x0000000d | recorded | RECORDED |
| 3d | BCONF | 0x0000003d | recorded | RECORDED |
| 3e | STAT.R256 after MODE_256_BIT_OUTPUT is written | 1 | 1 | PASS |
| 3f | which value of BCONF bit 3 means 256 bits | 1 means 256-bit | derived from 3a and 3d | RECORDED |
| 3g | FEATURES bits 1:0 and BCONF bit 2, the widest output each says the block was built for | max_rand_length=1 max_prng_len=1 | max_rand_length=1 max_prng_len=1 (both 256-bit), as read in 3c and 3d | PASS |
| 4a | RAND_RDY with RAND_GENERATING, RAND_RDY acknowledged first (1000 trials) | together=0 together_loose=1 timeouts=0 | timeouts=0; together recorded | PASS |
| 4b | RAND_RDY with RAND_GENERATING, second GENERATE over a latched RAND_RDY | together=8775 together_loose=9000 accepted=1000 words_changed=1000 timeouts=0 | timeouts=0; together recorded | PASS |
| 5a | STAT bits outside every field the crate names, ever seen | 0x00000000 | recorded | RECORDED |
| 5b | LAST_RESEED values seen (bit n set means value n was seen) | 0x11 | recorded | RECORDED |
| 5c | STAT bits 19 to 23 (above a 3-bit LAST_RESEED) ever set | 0 | recorded | RECORDED |
| 6a | generate to RAND_RDY, ticks | minimum=23 median=24 p99=25 maximum=25 timeouts=0 | timeouts=0 | PASS |
| 6b | generate to RAND_RDY, microseconds (median, maximum) | 6.0 6.25 | recorded | RECORDED |
| 6c | reseed to SEED_DONE, ticks | minimum=149 median=174 p99=203 maximum=203 timeouts=0 | timeouts=0 | PASS |
| 6d | reseed to SEED_DONE, microseconds (median, maximum) | 43.5 50.75 | recorded | RECORDED |
| 6e | one read of the time CSR, minimum ticks | 1 | recorded | RECORDED |
| 7a | ISTAT after the bring-up clear | 0x00000000 | no documented bit left | PASS |
| 7b | ISTAT bit 3 with AUTO_RQSTS=1 | latched=0 readback=0x1 istat_seen=0x00000001 istat_residue=0x00000000 | recorded; residue has no documented bit | PASS |
| 7c | ISTAT bit 2 with AUTO_AGE=1 | latched=1 readback=0x1 istat_seen=0x00000004 istat_residue=0x00000000 | recorded; residue has no documented bit | PASS |
| 8a | long run of driver-path generations | generations=1048576 ready=1048576 all_zero=0 lockups=0 unseeded=0 wrong_output_width=0 timeouts=0 | all_zero=0 wrong_output_width=0 timeouts=0 | PASS |
| 8b | first LFSR_LOCKUP: does a reseed clear it, and does the next generation serve | none seen | next_outcome=ready, or none seen | RECORDED |
| 9a | capture: bytes whose records verified | bytes=1310720 | bytes>=1048576 | PASS |
| 9b | FIPS 140-2 blocks failed | fips_failed=2 fips_blocks=524 | at most 3 | PASS |
| 9c | Shannon entropy, bits per byte | entropy=7.999854 | at least 7.99 | PASS |
| 9d | chi-square over byte values, upper-tail p | chi_square=264.17 chi_square_p=0.3333 | between 0.001 and 0.999 | PASS |
| 9e | mean byte and lag-1 serial correlation | mean=127.5159 serial_correlation=-0.000949 | mean within 0.5 of 127.5, abs(r) < 0.005 | PASS |
| 9f | repeated 32-byte generations in the capture | repeated_generations=0 | 0 | PASS |
| 9g | outside statistical suite, if one was run (name and summary) | not run | recorded | RECORDED |
