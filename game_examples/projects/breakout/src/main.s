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
    jsr setup_brick_attributes
    jsr init_game
    jsr draw_all_bricks
    jsr draw_hud

    lda #$00
    sta $2005
    sta $2005

    lda #$00
    sta $2000
    lda #$1E
    sta $2001

main_loop:
    jsr read_controller
    jsr update_game
    jsr draw_oam

    jsr wait_vblank
    jsr apply_bg_updates
    jsr apply_status_updates
    jsr apply_hud_updates

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

.proc setup_brick_attributes
    bit $2002
    lda #$23
    sta $2006
    lda #$C8
    sta $2006

    ldx #$08
    lda #$50
@loop:
    sta $2007
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
    sta bricks_bits + 1
    sta bricks_bits + 2
    sta bricks_bits + 3

    lda #$03
    sta lives
    lda #$00
    sta score
    sta game_state

    lda #$00
    lda #$00
    sta brick_dirty_flag
    sta status_dirty_flag
    sta hud_dirty_flag
    rts
.endproc

.proc draw_all_bricks
    ldx #$00
@loop:
    cpx #$20
    beq @done

    jsr set_brick_ppu_addr
    jsr set_brick_tile_base

    bit $2002
    lda temp_addr_hi
    sta $2006
    lda temp_addr_lo
    sta $2006
    lda temp_tile_base
    sta $2007
    clc
    adc #$01
    sta $2007
    sta $2007
    clc
    adc #$01
    sta $2007

    inx
    jmp @loop
@done:
    rts
.endproc

.proc set_brick_tile_base
    txa
    lsr a
    lsr a
    lsr a
    tay
    lda brick_tile_base_tbl, y
    sta temp_tile_base
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

.proc apply_status_updates
    lda status_dirty_flag
    beq @done

    lda status_kind
    cmp #$01
    bne @clear_flag
    jsr draw_clear_text

@clear_flag:
    lda #$00
    sta status_dirty_flag
@done:
    rts
.endproc

.proc apply_hud_updates
    lda hud_dirty_flag
    beq @done
    jsr draw_hud
    lda #$00
    sta hud_dirty_flag
@done:
    rts
.endproc

.proc draw_hud
    bit $2002
    lda #$20
    sta $2006
    lda #$01
    sta $2006

    ldx #$00
@life_loop:
    txa
    cmp lives
    bcs @life_empty
    lda #$02
    bne @life_write
@life_empty:
    lda #$00
@life_write:
    sta $2007
    inx
    cpx #$03
    bne @life_loop

    jsr calc_score_digits

    bit $2002
    lda #$20
    sta $2006
    lda #$19
    sta $2006

    lda score_hundreds
    clc
    adc #$0F
    sta $2007

    lda score_tens
    clc
    adc #$0F
    sta $2007

    lda score_ones
    clc
    adc #$0F
    sta $2007
    rts
.endproc

.proc calc_score_digits
    lda score
    ldx #$00
@hundreds_loop:
    cmp #$64
    bcc @hundreds_done
    sec
    sbc #$64
    inx
    jmp @hundreds_loop
@hundreds_done:
    stx score_hundreds

    ldx #$00
@tens_loop:
    cmp #$0A
    bcc @tens_done
    sec
    sbc #$0A
    inx
    jmp @tens_loop
@tens_done:
    stx score_tens
    sta score_ones
    rts
.endproc

.proc draw_clear_text
    bit $2002
    lda #$21
    sta $2006
    lda #$EC
    sta $2006

    lda #$19
    sta $2007
    lda #$1A
    sta $2007
    lda #$1B
    sta $2007
    lda #$1C
    sta $2007
    lda #$1D
    sta $2007
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
    lda game_state
    beq @active
    rts

@active:
    lda pad_state
    and #$02
    beq @check_right
    lda paddle_x
    sec
    sbc #$02
    cmp #$08
    bcs @store_left
    lda #$08
@store_left:
    sta paddle_x

@check_right:
    lda pad_state
    and #$01
    beq @move_ball
    lda paddle_x
    clc
    adc #$02
    cmp #$E8
    bcc @store_right
    lda #$E7
@store_right:
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
    lda lives
    bne @has_lives
    rts

@has_lives:
    dec lives

    lda #$01
    sta hud_dirty_flag

    lda lives
    bne @respawn

    lda #$02
    sta game_state
    rts

@respawn:
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

    lda ball_x
    clc
    adc #$04
    sta temp_center_x

    lda paddle_x
    clc
    adc #$08
    sta temp_zone

    lda temp_center_x
    cmp temp_zone
    bcc @paddle_left

    lda paddle_x
    clc
    adc #$10
    sta temp_zone
    lda temp_center_x
    cmp temp_zone
    bcs @paddle_right
    jmp @paddle_done

@paddle_left:
    lda #$FF
    sta ball_vx
    jmp @paddle_done

@paddle_right:
    lda #$01
    sta ball_vx

@paddle_done:
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
    cmp #$20
    bcc @no_hit
    cmp #$40
    bcs @no_hit

    sec
    sbc #$20
    lsr a
    lsr a
    lsr a
    sta temp_row_idx

    lda temp_center_x
    lsr a
    lsr a
    lsr a
    lsr a
    lsr a
    sta temp_col_idx

    lda temp_row_idx
    asl a
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

    inc score
    lda #$01
    sta hud_dirty_flag

    lda bricks_bits
    ora bricks_bits + 1
    ora bricks_bits + 2
    ora bricks_bits + 3
    bne @bounce

    lda #$01
    sta game_state
    sta status_kind
    sta status_dirty_flag

@bounce:
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
    .byte $0F, $2A, $2C, $30
    .byte $0F, $24, $28, $30
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
    .byte $80, $84, $88, $8C, $90, $94, $98, $9C
    .byte $A0, $A4, $A8, $AC, $B0, $B4, $B8, $BC
    .byte $C0, $C4, $C8, $CC, $D0, $D4, $D8, $DC
    .byte $E0, $E4, $E8, $EC, $F0, $F4, $F8, $FC

brick_nt_hi_tbl:
    .byte $20, $20, $20, $20, $20, $20, $20, $20
    .byte $20, $20, $20, $20, $20, $20, $20, $20
    .byte $20, $20, $20, $20, $20, $20, $20, $20
    .byte $20, $20, $20, $20, $20, $20, $20, $20

brick_tile_base_tbl:
    .byte $03, $06, $09, $0C

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
lives:
    .res 1
score:
    .res 1
game_state:
    .res 1
pad_state:
    .res 1
bricks_bits:
    .res 4
temp_mask:
    .res 1
oam_ptr:
    .res 1
brick_dirty_flag:
    .res 1
brick_dirty_index:
    .res 1
status_dirty_flag:
    .res 1
status_kind:
    .res 1
hud_dirty_flag:
    .res 1
temp_addr_lo:
    .res 1
temp_addr_hi:
    .res 1
temp_tile_base:
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
temp_zone:
    .res 1
score_tens:
    .res 1
score_hundreds:
    .res 1
score_ones:
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

; tile 3-5: brick row 0 (color index 1)
    .byte $00, $7F, $41, $5F, $5F, $41, $7F, $00 ; left
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $FF, $81, $FF, $FF, $81, $FF, $00 ; mid
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $FE, $82, $FA, $FA, $82, $FE, $00 ; right
    .byte $00, $00, $00, $00, $00, $00, $00, $00

; tile 6-8: brick row 1 (color index 2)
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $7F, $41, $5F, $5F, $41, $7F, $00 ; left
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $FF, $81, $FF, $FF, $81, $FF, $00 ; mid
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $FE, $82, $FA, $FA, $82, $FE, $00 ; right

; tile 9-11: brick row 2 (color index 3)
    .byte $00, $7F, $41, $5F, $5F, $41, $7F, $00 ; left
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $FF, $81, $FF, $FF, $81, $FF, $00 ; mid
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $00, $FE, $82, $FA, $FA, $82, $FE, $00 ; right
    .byte $00, $00, $00, $00, $00, $00, $00, $00

; tile 12-14: brick row 3 (color index 2)
    .byte $00, $00, $00, $00, $00, $00, $00, $00 ; left
    .byte $00, $7F, $41, $5F, $5F, $41, $7F, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00 ; mid
    .byte $00, $FF, $81, $FF, $FF, $81, $FF, $00
    .byte $00, $00, $00, $00, $00, $00, $00, $00 ; right
    .byte $00, $FE, $82, $FA, $FA, $82, $FE, $00

; tile 15-24: digits 0-9
    .byte $7E, $66, $66, $66, $66, $66, $7E, $00 ; 0
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $18, $38, $18, $18, $18, $18, $7E, $00 ; 1
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $06, $06, $7E, $60, $60, $7E, $00 ; 2
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $06, $06, $3E, $06, $06, $7E, $00 ; 3
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $66, $66, $66, $7E, $06, $06, $06, $00 ; 4
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $60, $60, $7E, $06, $06, $7E, $00 ; 5
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $60, $60, $7E, $66, $66, $7E, $00 ; 6
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $06, $06, $0C, $18, $18, $18, $00 ; 7
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $66, $66, $7E, $66, $66, $7E, $00 ; 8
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $66, $66, $7E, $06, $06, $7E, $00 ; 9
    .byte $00, $00, $00, $00, $00, $00, $00, $00

; tile 25-29: C L E A R
    .byte $3C, $66, $60, $60, $60, $66, $3C, $00 ; C
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $60, $60, $60, $60, $60, $60, $7E, $00 ; L
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7E, $60, $60, $7C, $60, $60, $7E, $00 ; E
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $18, $3C, $66, $66, $7E, $66, $66, $00 ; A
    .byte $00, $00, $00, $00, $00, $00, $00, $00
    .byte $7C, $66, $66, $7C, $78, $6C, $66, $00 ; R
    .byte $00, $00, $00, $00, $00, $00, $00, $00

    .res $1E20, $00
