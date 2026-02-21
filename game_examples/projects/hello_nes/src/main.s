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
    lda #$08
    sta $2001

forever:
    jmp forever
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

    .res $1F70, $00
