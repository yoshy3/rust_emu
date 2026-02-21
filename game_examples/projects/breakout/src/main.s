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

    jsr wait_vblank
    jsr wait_vblank

    jsr load_palettes
    jsr clear_nametable
    jsr init_game
    jsr draw_all_bricks

    lda #$00
    sta $2005
    sta $2005

    lda #$00
    sta $2000
    lda #$18
    sta $2001

main_loop:
    jsr read_controller
    jsr update_game
    jsr draw_oam

    jsr wait_vblank
    jsr apply_bg_updates

    lda #$00
    sta $2005
    sta $2005

    lda #$00
    sta $2003
    lda #$03
    sta $4014

    jmp main_loop
.endproc

.proc wait_vblank
@wait:
    bit $2002
    bpl @wait
    rts
.endproc

.proc load_palettes
    bit $2002
    lda #$3F
    sta $2006
    lda #$00
    sta $2006

    ldx #$00
@loop:
    lda palettes, x
    sta $2007
    inx
    cpx #$20
    bne @loop
    rts
.endproc

.proc clear_nametable
    bit $2002
    lda #$20
    sta $2006
    lda #$00
    sta $2006

    lda #$00
    ldx #$04
    ldy #$00
@loop:
    sta $2007
    iny
    bne @loop
    dex
    bne @loop
    rts
.endproc

.proc init_game
    lda #$70
    sta paddle_x
    lda #$D0
    sta paddle_y

    jsr reset_ball

    lda #$FF
    sta bricks_bits
    lda #$0F
    sta bricks_bits + 1

    lda #$00
    sta brick_dirty_flag
    rts
.endproc

.proc draw_all_bricks
    ldx #$00
@loop:
    cpx #$0C
    beq @done

    jsr set_brick_ppu_addr

    bit $2002
    lda temp_addr_hi
    sta $2006
    lda temp_addr_lo
    sta $2006
    lda #$03
    sta $2007
    sta $2007

    inx
    jmp @loop
@done:
    rts
.endproc

.proc apply_bg_updates
    lda brick_dirty_flag
    beq @done

    ldx brick_dirty_index
    jsr erase_brick_bg

    lda #$00
    sta brick_dirty_flag
@done:
    rts
.endproc

.proc set_brick_ppu_addr
    lda brick_nt_lo_tbl, x
    sta temp_addr_lo
    lda brick_nt_hi_tbl, x
    sta temp_addr_hi
    rts
.endproc

.proc erase_brick_bg
    jsr set_brick_ppu_addr

    bit $2002
    lda temp_addr_hi
    sta $2006
    lda temp_addr_lo
    sta $2006
    lda #$00
    sta $2007
    sta $2007
    rts
.endproc


.proc reset_ball
    lda #$78
    sta ball_x
    lda #$C0
    sta ball_y
    lda #$01
    sta ball_vx
    lda #$FF
    sta ball_vy
    rts
.endproc

.proc read_controller
    lda #$01
    sta $4016
    lda #$00
    sta $4016

    lda #$00
    sta pad_state

    ldx #$08
@loop:
    lda $4016
    and #$01
    lsr a
    rol pad_state
    dex
    bne @loop
    rts
.endproc

.proc update_game
    lda pad_state
    and #$02
    beq @check_right
    lda paddle_x
    cmp #$08
    bcc @check_right
    sec
    sbc #$02
    sta paddle_x

@check_right:
    lda pad_state
    and #$01
    beq @move_ball
    lda paddle_x
    cmp #$E8
    bcs @move_ball
    clc
    adc #$02
    sta paddle_x

@move_ball:
    clc
    lda ball_x
    adc ball_vx
    sta ball_x

    clc
    lda ball_y
    adc ball_vy
    sta ball_y

    lda ball_x
    cmp #$08
    bcs @check_right_wall
    lda #$08
    sta ball_x
    lda #$01
    sta ball_vx

@check_right_wall:
    lda ball_x
    cmp #$F8
    bcc @check_top_wall
    lda #$F8
    sta ball_x
    lda #$FF
    sta ball_vx

@check_top_wall:
    lda ball_y
    cmp #$10
    bcs @check_bottom
    lda #$10
    sta ball_y
    lda #$01
    sta ball_vy

@check_bottom:
    lda ball_y
    cmp #$E8
    bcc @check_paddle
    jsr reset_ball
    rts

@check_paddle:
    lda ball_vy
    bmi @check_bricks

    lda ball_y
    clc
    adc #$07
    cmp paddle_y
    bcc @check_bricks

    lda ball_y
    cmp paddle_y
    bcs @check_bricks

    lda ball_x
    clc
    adc #$07
    cmp paddle_x
    bcc @check_bricks

    lda paddle_x
    clc
    adc #$18
    cmp ball_x
    bcc @check_bricks

    lda #$FF
    sta ball_vy
    lda paddle_y
    sec
    sbc #$08
    sta ball_y

@check_bricks:
    jsr check_brick_hit

@done:
    rts
.endproc

.proc check_brick_hit
    lda ball_x
    clc
    adc #$04
    sta temp_center_x

    lda ball_y
    clc
    adc #$04
    sta temp_center_y

    lda temp_center_y
    cmp #$28
    bcc @no_hit
    cmp #$50
    bcs @no_hit

    sec
    sbc #$28
    sta temp_dy
    and #$0F
    cmp #$08
    bcs @no_hit

    lda temp_dy
    lsr a
    lsr a
    lsr a
    lsr a
    sta temp_row_idx

    lda temp_center_x
    cmp #$28
    bcc @no_hit
    cmp #$98
    bcs @no_hit

    sec
    sbc #$28
    sta temp_dx
    and #$1F
    cmp #$10
    bcs @no_hit

    lda temp_dx
    lsr a
    lsr a
    lsr a
    lsr a
    lsr a
    sta temp_col_idx

    lda temp_row_idx
    asl a
    asl a
    clc
    adc temp_col_idx
    tax

    jsr brick_is_alive
    beq @no_hit

    jsr brick_clear
    stx brick_dirty_index
    lda #$01
    sta brick_dirty_flag

    lda ball_vy
    eor #$FF
    clc
    adc #$01
    sta ball_vy

@no_hit:
    rts
.endproc

.proc brick_is_alive
    txa
    pha
    and #$07
    tay
    lda bit_mask_tbl, y
    sta temp_mask

    pla
    lsr a
    lsr a
    lsr a
    tay

    lda bricks_bits, y
    and temp_mask
    rts
.endproc

.proc brick_clear
    txa
    pha
    and #$07
    tay
    lda bit_mask_tbl, y
    eor #$FF
    sta temp_mask

    pla
    lsr a
    lsr a
    lsr a
    tay

    lda bricks_bits, y
    and temp_mask
    sta bricks_bits, y
    rts
.endproc

.proc draw_oam
    ldy #$00
    sty oam_ptr

    lda paddle_y
    sta $0300, y
    lda #$01
    sta $0301, y
    lda #$00
    sta $0302, y
    lda paddle_x
    sta $0303, y

    iny
    iny
    iny
    iny

    lda paddle_y
    sta $0300, y
    lda #$01
    sta $0301, y
    lda #$00
    sta $0302, y
    lda paddle_x
    clc
    adc #$08
    sta $0303, y

    iny
    iny
    iny
    iny

    lda paddle_y
    sta $0300, y
    lda #$01
    sta $0301, y
    lda #$00
    sta $0302, y
    lda paddle_x
    clc
    adc #$10
    sta $0303, y

    iny
    iny
    iny
    iny

    lda ball_y
    sta $0300, y
    lda #$02
    sta $0301, y
    lda #$00
    sta $0302, y
    lda ball_x
    sta $0303, y

    iny
    iny
    iny
    iny
    sty oam_ptr

@hide_rest:
    ldy oam_ptr
    lda #$F8
@hide_loop:
    sta $0300, y
    iny
    iny
    iny
    iny
    bne @hide_loop
    rts
.endproc

.proc nmi
    rti
.endproc

.proc irq
    rti
.endproc

.segment "RODATA"
palettes:
    .byte $0F, $16, $27, $30
    .byte $0F, $0F, $0F, $0F
    .byte $0F, $0F, $0F, $0F
    .byte $0F, $0F, $0F, $0F

    .byte $0F, $30, $21, $11
    .byte $0F, $16, $27, $18
    .byte $0F, $30, $10, $00
    .byte $0F, $2A, $12, $02

bit_mask_tbl:
    .byte $01, $02, $04, $08, $10, $20, $40, $80

brick_x_tbl:
    .byte $28, $48, $68, $88
    .byte $28, $48, $68, $88
    .byte $28, $48, $68, $88

brick_y_tbl:
    .byte $28, $28, $28, $28
    .byte $38, $38, $38, $38
    .byte $48, $48, $48, $48

brick_nt_lo_tbl:
    .byte $A5, $A9, $AD, $B1
    .byte $E5, $E9, $ED, $F1
    .byte $25, $29, $2D, $31

brick_nt_hi_tbl:
    .byte $20, $20, $20, $20
    .byte $20, $20, $20, $20
    .byte $21, $21, $21, $21

.segment "BSS"
paddle_x:
    .res 1
paddle_y:
    .res 1
ball_x:
    .res 1
ball_y:
    .res 1
ball_vx:
    .res 1
ball_vy:
    .res 1
pad_state:
    .res 1
bricks_bits:
    .res 2
temp_mask:
    .res 1
oam_ptr:
    .res 1
brick_dirty_flag:
    .res 1
brick_dirty_index:
    .res 1
temp_addr_lo:
    .res 1
temp_addr_hi:
    .res 1
temp_center_x:
    .res 1
temp_center_y:
    .res 1
temp_dx:
    .res 1
temp_dy:
    .res 1
temp_row_idx:
    .res 1
temp_col_idx:
    .res 1

.segment "VECTORS"
    .addr nmi
    .addr reset
    .addr irq

.segment "CHARS"
; tile 0: transparent
    .res 16, $00

; tile 1: paddle (solid)
    .byte $FF, $FF, $FF, $FF, $FF, $FF, $FF, $FF
    .byte $00, $00, $00, $00, $00, $00, $00, $00

; tile 2: ball
    .byte $00, $18, $3C, $3C, $3C, $3C, $18, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

; tile 3: brick
    .byte $FF, $FF, $81, $FF, $81, $FF, $FF, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .res $1FC0, $00
