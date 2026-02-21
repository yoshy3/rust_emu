.segment "HEADER"
    .byte 'N', 'E', 'S', $1A
    .byte 2
    .byte 1
    .byte $00
    .byte $00
    .res 8, $00

.segment "STARTUP"
.proc reset
    sei
    cld
    ldx #$40
    stx $4017
    ldx #$FF
    txs
    inx
    stx $2000
    stx $2001
    stx $4010

vblankwait1:
    bit $2002
    bpl vblankwait1

vblankwait2:
    bit $2002
    bpl vblankwait2

    lda #$3F
    sta $2006
    lda #$00
    sta $2006
    lda #$0F
    sta $2007
    lda #$30
    sta $2007
    lda #$16
    sta $2007
    lda #$27
    sta $2007

    lda #$3F
    sta $2006
    lda #$10
    sta $2006
    ldx #$00
load_sprite_palette:
    lda sprite_palette, x
    sta $2007
    inx
    cpx #$10
    bne load_sprite_palette

    lda #$20
    sta $2006
    lda #$00
    sta $2006
    lda #$00
    ldy #$00
    ldx #$04
clear_nametable:
    sta $2007
    iny
    bne clear_nametable
    dex
    bne clear_nametable

    lda #$21
    sta $2006
    lda #$CB
    sta $2006
    ldx #$00
draw_text:
    lda hello_text, x
    beq draw_done
    sta $2007
    inx
    bne draw_text
draw_done:

    lda #$00
    sta $2005
    sta $2005

    lda #$00
    sta $2000
    lda #$18
    sta $2001

    lda #$A7
    sta rand_seed

main_loop:
vblankwait3:
    bit $2002
    bpl vblankwait3

    jsr draw_stars
    inc frame_counter

forever:
    jmp main_loop
.endproc

.proc draw_stars
    lda #$00
    sta $2003

    ldx #$00
draw_stars_loop:
    stx temp_i

    lda rand_seed
    asl a
    bcc lfsr_no_xor
    eor #$1D
lfsr_no_xor:
    sta rand_seed
    sta temp_rand

    txa
    asl a
    clc
    adc frame_counter
    clc
    adc temp_rand
    sta $2004

    lda temp_i
    eor temp_rand
    and #$03
    clc
    adc #$09
    sta $2004

    lda temp_rand
    lsr a
    and #$03
    sta $2004

    lda temp_i
    asl a
    asl a
    clc
    adc frame_counter
    clc
    adc temp_rand
    sta $2004

    ldx temp_i
    inx
    cpx #$40
    bne draw_stars_loop

    rts
.endproc

.proc nmi
    rti
.endproc

.proc irq
    rti
.endproc

.segment "RODATA"
hello_text:
    .byte $01, $02, $03, $03, $04, $05, $08, $06, $02, $07, $00

sprite_palette:
    .byte $0F, $30, $16, $27
    .byte $0F, $2A, $21, $11
    .byte $0F, $36, $17, $28
    .byte $0F, $12, $22, $32

.segment "BSS"
frame_counter:
    .res 1
rand_seed:
    .res 1
temp_rand:
    .res 1
temp_i:
    .res 1

.segment "VECTORS"
    .addr nmi
    .addr reset
    .addr irq

.segment "CHARS"
    .res 16, $00

    .byte $82, $82, $82, $FE, $82, $82, $82, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $FE, $80, $80, $FE, $80, $80, $FE, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $80, $80, $80, $80, $80, $80, $FE, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $7E, $82, $82, $82, $82, $82, $7E, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $00, $00, $00, $00, $00, $18, $18, $30
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $82, $C2, $A2, $92, $8A, $86, $82, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $7E, $82, $80, $7E, $02, $82, $7E, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .res 16, $00

    .byte $00, $10, $54, $38, $FE, $38, $54, $10
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $00, $10, $38, $7C, $38, $10, $00, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $00, $28, $10, $FE, $10, $28, $00, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .byte $44, $28, $10, $28, $44, $00, $00, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .res $1F30, $00
