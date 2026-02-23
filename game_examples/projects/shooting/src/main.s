.segment "HEADER"
    .byte 'N', 'E', 'S', $1A
    .byte 2  ; 32KB PRG
    .byte 1  ; 8KB CHR
    .byte $00
    .byte $00
    .res 8, $00

.segment "ZP"
pad_state: .res 1
player_x:  .res 1
player_y:  .res 1
nmi_ready: .res 1
bullet_cooldown: .res 1
player_lives: .res 1
player_invincible: .res 1
game_state:    .res 1

; For bullets
bullet_active: .res 4
bullet_x:      .res 4
bullet_y:      .res 4

; For enemy
enemy_active:  .res 10
enemy_x:       .res 10
enemy_y:       .res 10
enemy_cooldown:.res 10
enemy_type:    .res 10
enemy_state:   .res 10
enemy_base_x:  .res 10

; For Waves
wave_number:   .res 1
wave_timer_lo: .res 1
wave_timer_hi: .res 1
spawn_timer:   .res 1

; For enemy bullets
enemy_bullet_active: .res 8
enemy_bullet_x:      .res 8
enemy_bullet_y:      .res 8
enemy_bullet_vx:     .res 8
enemy_bullet_vy:     .res 8

global_frame_counter: .res 1
draw_score_flag:      .res 1
score_digits:         .res 6

.segment "BSS"
OAM_RAM = $0200

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

    ; Clear OAM
    ldx #$00
    lda #$FF
@clear_oam:
    sta OAM_RAM, x
    inx
    bne @clear_oam

    ; Load Palettes
    bit $2002
    lda #$3F
    sta $2006
    lda #$00
    sta $2006
    ; Fill Palettes
    ldx #$20
    lda #$0F
@clear_palettes:
    sta $2007
    dex
    bne @clear_palettes

    ; Re-write Background Palette
    lda #$3F
    sta $2006
    lda #$00
    sta $2006
    lda #$0F
    sta $2007
    lda #$00
    sta $2007
    lda #$10
    sta $2007
    lda #$30
    sta $2007

    ; Re-write Sprite Palette 0 (Addresses 3F10)
    lda #$3F
    sta $2006
    lda #$10
    sta $2006
    lda #$0F       ; Transparent
    sta $2007
    lda #$16       ; Color 1 (Red)
    sta $2007
    lda #$27       ; Color 2 (Orange)
    sta $2007
    lda #$30       ; Color 3 (White)
    sta $2007

    ; Sprite Palette 1 (3F14) - Enemy Straight (Blue)
    lda #$3F
    sta $2006
    lda #$14
    sta $2006
    lda #$0F       ; Transparent
    sta $2007
    lda #$12       ; Blue
    sta $2007
    lda #$21       ; Light blue
    sta $2007
    lda #$11       ; Cyan
    sta $2007

    ; Sprite Palette 2 (3F18) - Enemy Diagonal (Green)
    lda #$3F
    sta $2006
    lda #$18
    sta $2006
    lda #$0F       ; Transparent
    sta $2007
    lda #$1A       ; Dark Green
    sta $2007
    lda #$2A       ; Green
    sta $2007
    lda #$19       ; Yellow Green
    sta $2007

    ; Sprite Palette 3 (3F1C) - Enemy Sine (Purple/Red)
    lda #$3F
    sta $2006
    lda #$1C
    sta $2006
    lda #$0F       ; Transparent
    sta $2007
    lda #$14       ; Purple
    sta $2007
    lda #$24       ; Pink
    sta $2007
    lda #$25       ; Light Pink/Red
    sta $2007

    lda #$80
    sta player_x
    lda #$C0
    sta player_y
    lda #$00
    sta nmi_ready
    sta bullet_cooldown
    sta player_invincible
    lda #$03
    sta player_lives
    lda #$00
    sta game_state

    ldx #$00
@clear_bullets:
    sta bullet_active, x
    inx
    cpx #$04
    bcc @clear_bullets

    ; Init enemies
    ldx #$00
    lda #$00
@clear_enemies:
    sta enemy_active, x
    inx
    cpx #10
    bcc @clear_enemies

    ; Init enemy bullets
    ldx #$00
    lda #$00
@clear_ebullets:
    sta enemy_bullet_active, x
    sta enemy_bullet_vx, x
    sta enemy_bullet_vy, x
    inx
    cpx #8
    bcc @clear_ebullets

    ; Init waves
    lda #$00
    sta wave_number
    sta wave_timer_lo
    sta wave_timer_hi
    lda #120      ; initial delay before wave 0 starts
    sta spawn_timer

    lda #$00
    sta global_frame_counter
    sta draw_score_flag
    ldx #$00
@clear_score:
    sta score_digits, x
    inx
    cpx #6
    bcc @clear_score
    
    lda #$01
    sta draw_score_flag

    ; Enable NMI and rendering
    lda #$80      ; changed from $88 so sprite table is $0000
    sta $2000
    lda #$1E
    sta $2001

main_loop:
    inc nmi_ready
    inc global_frame_counter
@wait:
    lda nmi_ready
    bne @wait

    jsr read_controller
    
    lda game_state
    bne @game_over

    jsr update_wave
    jsr spawn_enemy
    jsr update_player
    jsr update_bullets
    jsr update_enemy
    jsr update_enemy_bullets
    jsr check_collisions
    jsr draw_player
    jsr draw_enemy
    jsr draw_enemy_bullets
    jsr draw_ui

    jmp main_loop

@game_over:
    ; Check for B button ($40) to restart
    lda pad_state
    and #$40
    beq @keep_game_over
    ; Restart game
    jmp reset

@keep_game_over:
    ; Hide all sprites
    jsr hide_all_sprites
    jmp main_loop
.endproc

.proc hide_all_sprites
    ldx #$00
    lda #$FF
@hide_loop:
    sta OAM_RAM, x
    inx
    inx
    inx
    inx
    bne @hide_loop
    rts
.endproc

.proc nmi
    pha
    txa
    pha
    tya
    pha

    lda nmi_ready
    beq @skip_update

    ; OAM DMA
    lda #$00
    sta $2003
    lda #$02
    sta $4014

    lda #$00
    sta $2005
    sta $2005

    ; Draw Score
    lda draw_score_flag
    beq @skip_score_draw

    lda $2002
    lda #$20
    sta $2006
    lda #$20
    sta $2006
    
    ldx #5
@score_loop:
    lda score_digits, x
    clc
    adc #$10
    sta $2007
    dex
    bpl @score_loop

    lda #$00
    sta draw_score_flag

    ; reset scroll
    lda #$00
    sta $2005
    sta $2005

@skip_score_draw:

    ; Reset NMI flag
    dec nmi_ready

@skip_update:
    pla
    tay
    pla
    tax
    pla
    rti
.endproc

.proc wait_vblank
@wait:
    bit $2002
    bpl @wait
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

.proc update_player
    ; Invincibility frames
    lda player_invincible
    beq @skip_inv
    dec player_invincible
@skip_inv:

    lda bullet_cooldown
    beq @skip_cooldown
    dec bullet_cooldown
@skip_cooldown:

    ; Move Right
    lda pad_state
    and #$01
    beq @not_right
    lda player_x
    clc
    adc #$02
    cmp #$F0
    bcs @skip_right
    sta player_x
@skip_right:
@not_right:

    ; Move Left
    lda pad_state
    and #$02
    beq @not_left
    lda player_x
    sec
    sbc #$02
    cmp #$08
    bcc @skip_left
    sta player_x
@skip_left:
@not_left:

    ; Move Down
    lda pad_state
    and #$04
    beq @not_down
    lda player_y
    clc
    adc #$02
    cmp #$E0
    bcs @skip_down
    sta player_y
@skip_down:
@not_down:

    ; Move Up
    lda pad_state
    and #$08
    beq @not_up
    lda player_y
    sec
    sbc #$02
    cmp #$10
    bcc @skip_up
    sta player_y
@skip_up:
@not_up:

    ; Fire bullets
    lda pad_state
    and #$80      ; A button
    beq @not_fire

    ; Check cooldown
    lda bullet_cooldown
    bne @not_fire

    ldx #$00
@find_bullet:
    lda bullet_active, x
    bne @next_bullet
    ; Found empty slot!
    lda #$01
    sta bullet_active, x
    
    lda #15       ; Cooldown counter (15 frames = 0.25s)
    sta bullet_cooldown

    lda player_x
    clc
    adc #$04
    sta bullet_x, x
    lda player_y
    sec
    sbc #$04
    sta bullet_y, x
    jmp @not_fire     ; only fire one per frame
@next_bullet:
    inx
    cpx #$04
    bcc @find_bullet

@not_fire:
    rts
.endproc

.proc update_bullets
    ldx #$00
@loop:
    lda bullet_active, x
    beq @skip
    
    lda bullet_y, x
    sec
    sbc #$04
    sta bullet_y, x
    
    ; Off screen (moved up past 0 -> wraps to > $F0)
    cmp #$F0
    bcc @skip
    lda #$00
    sta bullet_active, x

@skip:
    inx
    cpx #$04
    bcc @loop
    rts
.endproc

.proc draw_player
    ; Blink if invincible
    lda player_invincible
    beq @draw
    and #$04      ; Blink speed (every 4 frames)
    bne @draw
    ; Hide player
    lda #$FF
    sta OAM_RAM+0
    sta OAM_RAM+4
    sta OAM_RAM+8
    sta OAM_RAM+12
    jmp @draw_bullets

@draw:
    ; Sprite 0 - Top Left
    lda player_y
    sta OAM_RAM+0
    lda #$04      ; Tile 4
    sta OAM_RAM+1
    lda #$00      ; Palette 0
    sta OAM_RAM+2
    lda player_x
    sta OAM_RAM+3

    ; Sprite 1 - Top Right
    lda player_y
    sta OAM_RAM+4
    lda #$04
    sta OAM_RAM+5
    lda #$40      ; Flip H
    sta OAM_RAM+6
    lda player_x
    clc
    adc #$08
    sta OAM_RAM+7

    ; Sprite 2 - Bottom Left
    lda player_y
    clc
    adc #$08
    sta OAM_RAM+8
    lda #$05
    sta OAM_RAM+9
    lda #$00      ; No Flip
    sta OAM_RAM+10
    lda player_x
    sta OAM_RAM+11

    ; Sprite 3 - Bottom Right
    lda player_y
    clc
    adc #$08
    sta OAM_RAM+12
    lda #$05
    sta OAM_RAM+13
    lda #$40      ; Flip H
    sta OAM_RAM+14
    lda player_x
    clc
    adc #$08
    sta OAM_RAM+15

@draw_bullets:
    ; Draw Bullets
    ldx #$00
    ldy #$10      ; OAM offset for bullets (starts at sprite 4)
@bullet_loop:
    lda bullet_active, x
    bne @draw_bullet
    
    ; Hide bullet
    lda #$FF
    sta OAM_RAM, y
    jmp @next_bullet
    
@draw_bullet:
    lda bullet_y, x
    sta OAM_RAM, y
    lda #$02      ; Tile 2 for bullet
    sta OAM_RAM+1, y
    lda #$00      ; Palette 0
    sta OAM_RAM+2, y
    lda bullet_x, x
    sta OAM_RAM+3, y

@next_bullet:
    iny
    iny
    iny
    iny
    inx
    cpx #$04
    bcc @bullet_loop

    rts
.endproc

.proc update_wave
    ; increment wave timer
    inc wave_timer_lo
    bne @skip_hi
    inc wave_timer_hi
@skip_hi:
    ; 10 seconds = 600 frames = $0258
    ; check if high byte = $02 and low byte >= $58
    lda wave_timer_hi
    cmp #$02
    bcc @done
    lda wave_timer_lo
    cmp #$58
    bcc @done
    
    ; next wave! (reset timers)
    lda #$00
    sta wave_timer_lo
    sta wave_timer_hi
    
    inc wave_number
    lda wave_number
    cmp #$03        ; 3 waves max, then loop back to 0
    bcc @save_wave
    lda #$00
@save_wave:
    sta wave_number
@done:
    rts
.endproc

.proc spawn_enemy
    ; check spawn timer
    lda spawn_timer
    beq @do_spawn
    dec spawn_timer
    rts

@do_spawn:
    ; find empty slot
    ldx #$00
@loop:
    lda enemy_active, x
    beq @found
    inx
    cpx #10
    bcc @loop
    ; no empty slots, just return. Will try again next frame but we should reset spawn_timer so we don't try every frame if full? 
    ; It's fine to try every frame if full, once one dies it spawns immediately.
    rts

@found:
    ; Spawning!
    lda #$01
    sta enemy_active, x
    lda #$00
    sta enemy_y, x
    lda #30
    sta enemy_cooldown, x
    sta enemy_state, x    ; init state to 0 for sine wave

    ; determine wave properties
    lda wave_number
    cmp #$00
    beq @wave0
    cmp #$01
    beq @wave1
    jmp @wave2

@wave0:
    ; Wave 0: Straight down, medium frequency
    lda #60
    sta spawn_timer
    lda #$00
    sta enemy_type, x
    
    ; random X: $10 - $E0
    jsr get_random_x
    sta enemy_x, x
    sta enemy_base_x, x
    rts

@wave1:
    ; Wave 1: Diagonal, fast frequency
    lda #30
    sta spawn_timer
    ; random 1 or 2 (Diagonal Right vs Left)
    lda wave_timer_lo
    and #$01
    clc
    adc #$01
    sta enemy_type, x
    
    jsr get_random_x
    sta enemy_x, x
    sta enemy_base_x, x
    rts

@wave2:
    ; Wave 2: Sine wave, very fast frequency
    lda #20
    sta spawn_timer
    lda #$03
    sta enemy_type, x
    
    jsr get_random_x
    sta enemy_x, x
    sta enemy_base_x, x
    rts
.endproc

.proc get_random_x
    ; simple "random" x
    lda wave_timer_lo
    and #$7F      ; 0..127
    clc
    adc user_x_offset ; we need different x per spawn. nmi_ready alone might be same if spawned on same frame
    adc #$20      ; +32 -> 32..159 (safe on screen)
    inc user_x_offset
    rts
user_x_offset: .byte 0 ; sequence counter
.endproc

.proc update_enemy
    ldx #$00
@loop:
    lda enemy_active, x
    bne @update
    jmp @next
@update:
    ; Select movement based on type
    lda enemy_type, x
    cmp #$00
    beq @move_straight
    cmp #$01
    beq @move_diag_r
    cmp #$02
    beq @move_diag_l
    jmp @move_sine

@move_straight:
    lda enemy_y, x
    clc
    adc #$01
    sta enemy_y, x
    jmp @check_bounds

@move_diag_r:
    lda enemy_y, x
    clc
    adc #$01
    sta enemy_y, x
    lda enemy_x, x
    clc
    adc #$01
    sta enemy_x, x
    jmp @check_bounds

@move_diag_l:
    lda enemy_y, x
    clc
    adc #$01
    sta enemy_y, x
    lda enemy_x, x
    sec
    sbc #$01
    sta enemy_x, x
    jmp @check_bounds

@move_sine:
    ; Y moves down slowly (1 px per 2 frames or 1 px per frame)
    lda wave_timer_lo
    and #$01
    beq @skip_y
    lda enemy_y, x
    clc
    adc #$01
    sta enemy_y, x
@skip_y:
    ; X moves according to sine table
    ldy enemy_state, x
    lda sine_table, y
    clc
    adc enemy_base_x, x
    sta enemy_x, x
    
    ; update state
    iny
    cpy #64
    bcc @save_state
    ldy #0
@save_state:
    tya
    sta enemy_state, x
    jmp @check_bounds

@check_bounds:
    lda enemy_y, x
    cmp #$EF
    bcc @fire_check
    ; Offscreen
    lda #$00
    sta enemy_active, x
    jmp @next

@fire_check:
    lda enemy_cooldown, x
    beq @can_fire
    dec enemy_cooldown, x
    jmp @next

@can_fire:
    ; random chance to fire
    lda wave_timer_lo
    and #$0F
    beq @continue_fire
    jmp @reset_cooldown
@continue_fire:
    
    ; find empty bullet
    ldy #$00
@ebullet_loop:
    lda enemy_bullet_active, y
    beq @found_bullet
    jmp @ebullet_next
@found_bullet:
    
    ; fire!
    lda #$01
    sta enemy_bullet_active, y
    lda enemy_x, x
    clc
    adc #$04
    sta enemy_bullet_x, y
    lda enemy_y, x
    clc
    adc #$04
    sta enemy_bullet_y, y
    
    ; Calculate Aim
    ; dx = player_x - enemy_x
    ; dy = player_y - enemy_y
    txa
    pha           ; Save enemy index (x)
    
    lda player_x
    sec
    sbc enemy_x, x
    bcs @pos_x
    ; DX is negative
    eor #$FF
    clc
    adc #1
    sta OAM_RAM+244 ; temp abs_dx
    lda #$FF      ; -1 flag
    sta OAM_RAM+245 ; temp sign_x
    jmp @do_y
@pos_x:
    sta OAM_RAM+244 ; temp abs_dx
    lda #$01      ; +1 flag
    sta OAM_RAM+245 ; temp sign_x

@do_y:
    pla
    tax
    
    lda player_y
    sec
    sbc enemy_y, x
    bcs @pos_y
    eor #$FF
    clc
    adc #1
    sta OAM_RAM+246 ; temp abs_dy
    lda #$FF
    sta OAM_RAM+247 ; temp sign_y
    jmp @compare
@pos_y:
    sta OAM_RAM+246 ; temp abs_dy
    lda #$01
    sta OAM_RAM+247 ; temp sign_y

@compare:
    ; Compare abs_dx and abs_dy
    lda OAM_RAM+244
    cmp OAM_RAM+246
    bcc @dy_major
    
    ; DX is major. X speed = 2.
    lda OAM_RAM+245
    bpl @set_vx2_pos
    lda #$FE      ; -2
    sta enemy_bullet_vx, y
    jmp @calc_minor_dy
@set_vx2_pos:
    lda #$02      ; +2
    sta enemy_bullet_vx, y

@calc_minor_dy:
    ; Y speed based on minor/major ratio
    ; half_major = abs_dx >> 1
    lda OAM_RAM+244
    lsr a
    cmp OAM_RAM+246
    bcs @minor_y_1
    ; minor > half_major -> Y speed 2
    lda OAM_RAM+247
    bpl @set_vy2_pos
    lda #$FE
    sta enemy_bullet_vy, y
    jmp @aim_done
@set_vy2_pos:
    lda #$02
    sta enemy_bullet_vy, y
    jmp @aim_done
    
@minor_y_1:
    ; minor <= half_major -> Y speed 1
    lda OAM_RAM+247
    bpl @set_vy1_pos
    lda #$FF
    sta enemy_bullet_vy, y
    jmp @aim_done
@set_vy1_pos:
    lda #$01
    sta enemy_bullet_vy, y
    jmp @aim_done

@dy_major:
    ; DY is major. Y speed = 2.
    lda OAM_RAM+247
    bpl @set_vy22_pos
    lda #$FE
    sta enemy_bullet_vy, y
    jmp @calc_minor_dx
@set_vy22_pos:
    lda #$02
    sta enemy_bullet_vy, y

@calc_minor_dx:
    ; X speed
    lda OAM_RAM+246
    lsr a
    cmp OAM_RAM+244
    bcs @minor_x_1
    ; minor > half_major -> X speed 2
    lda OAM_RAM+245
    bpl @set_vx22_pos
    lda #$FE
    sta enemy_bullet_vx, y
    jmp @aim_done
@set_vx22_pos:
    lda #$02
    sta enemy_bullet_vx, y
    jmp @aim_done

@minor_x_1:
    lda OAM_RAM+245
    bpl @set_vx1_pos
    lda #$FF
    sta enemy_bullet_vx, y
    jmp @aim_done
@set_vx1_pos:
    lda #$01
    sta enemy_bullet_vx, y

@aim_done:
    jmp @reset_cooldown

@ebullet_next:
    iny
    cpy #8
    bcs @done_all_ebullets
    jmp @ebullet_loop
@done_all_ebullets:

@reset_cooldown:
    lda #30
    sta enemy_cooldown, x

@next:
    inx
    cpx #10
    bcs @done_loop
    jmp @loop
@done_loop:
    rts
.endproc

.proc update_enemy_bullets
    lda global_frame_counter
    and #$01
    bne @do_update
    rts
@do_update:
    ldx #$00
@loop:
    lda enemy_bullet_active, x
    beq @skip
    
    lda enemy_bullet_x, x
    clc
    adc enemy_bullet_vx, x
    sta enemy_bullet_x, x
    
    ; Despawn if leaving sides
    cmp #$F8
    bcs @despawn
    cmp #$04
    bcc @despawn
    
    lda enemy_bullet_y, x
    clc
    adc enemy_bullet_vy, x
    sta enemy_bullet_y, x
    
    ; Despawn if leaving top/bottom
    cmp #$F0
    bcs @despawn
    cmp #$04
    bcc @despawn
    jmp @skip
    
@despawn:
    lda #$00
    sta enemy_bullet_active, x

@skip:
    inx
    cpx #8
    bcc @loop
    rts
.endproc

.proc check_collisions
    ; Player bullet vs Enemy
    ldx #$00
@bullet_loop:
    lda bullet_active, x
    beq @bullet_next
    
    ldy #$00
@enemy_loop:
    lda enemy_active, y
    beq @enemy_next
    
    ; check y collision (bullet 8x8, enemy 16x16)
    ; bullet_y + 8 > enemy_y AND bullet_y < enemy_y + 16
    ; => bullet_y - enemy_y + 8 < 24
    lda bullet_y, x
    sec
    sbc enemy_y, y
    clc
    adc #$08
    cmp #24
    bcs @enemy_next
    
    ; check x collision
    lda bullet_x, x
    sec
    sbc enemy_x, y
    clc
    adc #$08
    cmp #24
    bcs @enemy_next
    
    ; collision!
    lda #$00
    sta enemy_active, y
    sta bullet_active, x
    
    txa
    pha
    tya
    pha
    jsr add_score
    pla
    tay
    pla
    tax
    
    jmp @bullet_next  ; bullet is gone, move to next player bullet

@enemy_next:
    iny
    cpy #10
    bcs @bullet_next
    jmp @enemy_loop

@bullet_next:
    inx
    cpx #$04
    bcs @check_player
    jmp @bullet_loop

@check_player:
    ; Player vs Enemy Body
    lda player_invincible
    bne @done_all
    
    ldy #$00
@player_enemy_loop:
    lda enemy_active, y
    beq @pe_next
    
    ; Player 2x2 (center +7 offset), Enemy 16x16
    ; (player_y + 7) - enemy_y + 2 < 18
    lda player_y
    clc
    adc #7
    sec
    sbc enemy_y, y
    clc
    adc #2
    cmp #18
    bcs @pe_next
    
    lda player_x
    clc
    adc #7
    sec
    sbc enemy_x, y
    clc
    adc #2
    cmp #18
    bcs @pe_next
    
    ; Hit!
    jmp player_hurt

@pe_next:
    iny
    cpy #10
    bcs @check_bullets
    jmp @player_enemy_loop

@check_bullets:
    ; Player vs Enemy Bullets
    ldy #$00
@player_ebullet_loop:
    lda enemy_bullet_active, y
    beq @peb_next
    
    ; Player 2x2 (center +7), Bullet 8x8
    ; (player_y + 7) - bullet_y + 2 < 10
    lda player_y
    clc
    adc #7
    sec
    sbc enemy_bullet_y, y
    clc
    adc #2
    cmp #10
    bcs @peb_next
    
    lda player_x
    clc
    adc #7
    sec
    sbc enemy_bullet_x, y
    clc
    adc #2
    cmp #10
    bcs @peb_next
    
    ; Hit!
    jmp player_hurt

@peb_next:
    iny
    cpy #8
    bcs @done_all
    jmp @player_ebullet_loop

@done_all:
    rts
.endproc

player_hurt:
    dec player_lives
    lda player_lives
    bne @alive
    ; Game Over
    lda #$01
    sta game_state
    rts
@alive:
    ; Reset position
    lda #$80
    sta player_x
    lda #$C0
    sta player_y
    ; Set invincibility
    lda #60
    sta player_invincible
    rts

.proc add_score
    ldx #2
@add_loop:
    inc score_digits, x
    lda score_digits, x
    cmp #10
    bcc @done
    lda #0
    sta score_digits, x
    inx
    cpx #6
    bcc @add_loop
@done:
    lda #1
    sta draw_score_flag
    rts
.endproc

.proc draw_enemy
    ldx #$00
    ldy #32
@loop:
    lda enemy_active, x
    bne @draw
    
    lda #$FF
    sta OAM_RAM, y
    sta OAM_RAM+4, y
    sta OAM_RAM+8, y
    sta OAM_RAM+12, y
    jmp @next
    
@draw:
    txa
    pha          ; Save X
    
    ; get type
    lda enemy_type, x
    tax          ; X = type
    lda enemy_palette_lookup, x
    sta OAM_RAM+2, y
    sta OAM_RAM+6, y
    sta OAM_RAM+10, y
    sta OAM_RAM+14, y
    
    lda enemy_tile_top_lookup, x
    sta OAM_RAM+1, y   ; Top Left
    sta OAM_RAM+5, y   ; Top Right (flipped)
    
    lda enemy_tile_bot_lookup, x
    sta OAM_RAM+9, y   ; Bottom Left
    sta OAM_RAM+13, y  ; Bottom Right (flipped)
    
    pla
    tax          ; Restore X (enemy index)

    ; Draw Top Left
    lda enemy_y, x
    sta OAM_RAM, y
    lda enemy_x, x
    sta OAM_RAM+3, y
    
    ; Draw Top Right
    lda enemy_y, x
    sta OAM_RAM+4, y
    lda #$40      ; Flip H
    sta OAM_RAM+6, y
    lda enemy_x, x
    clc
    adc #$08
    sta OAM_RAM+7, y
    
    ; Draw Bottom Left
    lda enemy_y, x
    clc
    adc #$08
    sta OAM_RAM+8, y
    lda #$00      ; No Flip V to match player style (tiles are designed pre-flipped vertically if needed, or we just design top and bottom. Player uses no flip for bottom left. Let's stick to 00)
    sta OAM_RAM+10, y
    lda enemy_x, x
    sta OAM_RAM+11, y
    
    ; Draw Bottom Right
    lda enemy_y, x
    clc
    adc #$08
    sta OAM_RAM+12, y
    lda #$40      ; Flip H
    sta OAM_RAM+14, y
    lda enemy_x, x
    clc
    adc #$08
    sta OAM_RAM+15, y

@next:
    tya
    clc
    adc #16
    tay
    inx
    cpx #10
    bcs @done_loop
    jmp @loop
@done_loop:
    rts
.endproc

.proc draw_enemy_bullets
    ldx #$00
    ldy #192
@loop:
    lda enemy_bullet_active, x
    bne @draw
    
    lda #$FF
    sta OAM_RAM, y
    jmp @next

@draw:
    lda enemy_bullet_y, x
    sta OAM_RAM, y
    lda #$02      ; Tile 2 for bullet
    sta OAM_RAM+1, y
    lda #$01      ; Palette 1
    sta OAM_RAM+2, y
    lda enemy_bullet_x, x
    sta OAM_RAM+3, y

@next:
    iny
    iny
    iny
    iny
    inx
    cpx #8
    bcc @loop
    rts
.endproc

.proc draw_ui
    ; Draw lives icons using last few OAM slots (OAM_RAM+240..)
    ; max 3 lives
    ldx #$00
    ldy #240
    lda player_lives
    beq @clear
@loop:
    cmp #$00
    beq @clear_rest
    
    ; draw life icon (small version, let's just use top-left of player, Tile 4)
    lda #$E8      ; Y: bottom screen
    sta OAM_RAM, y
    lda #$04      ; Tile 4
    sta OAM_RAM+1, y
    lda #$00      ; Palette 0
    sta OAM_RAM+2, y
    
    ; X: $08, $18, $28
    txa
    asl a
    asl a
    asl a
    asl a
    clc
    adc #$08
    sta OAM_RAM+3, y
    
    iny
    iny
    iny
    iny
    inx
    sec
    sbc #$01       ; A contains lives left, but we modified it. Let's rethink.
    pha
    txa
    cmp player_lives
    pla
    bcc @loop       ; if x < lives
    ; Fall through to clear rest

@clear_rest:
    cpx #$03
    bcs @done
@clear:
    lda #$FF
    sta OAM_RAM, y
    iny
    iny
    iny
    iny
    inx
    cpx #$03
    bcc @clear
@done:
    rts
.endproc

.segment "VECTORS"
    .word nmi, reset, 0

.segment "RODATA"
enemy_palette_lookup:
    .byte $01, $02, $02, $03 ; Straight = Pal 1, Diag R/L = Pal 2, Sine = Pal 3

enemy_tile_top_lookup:
    .byte $06, $08, $08, $0A

enemy_tile_bot_lookup:
    .byte $07, $09, $09, $0B

sine_table:
    ; 64 bytes for a full sine wave circle. Amplitude ~30 pixels.
    ; Offset applied: values center around 0. In assembly, we usually store absolute offsets to add or subtract, but for simplicity, let's make it signed or just add/subtract.
    ; Let's make it a signed additive offset table: values from 128-30 to 128+30, treat 128 as zero and subtract 128 before adding? 
    ; Better: just values from 0 to 60, and subtract 30 before adding to base.
    ; Or simpler: store raw values to add to base (0 to 60), but sine oscillates.
    ; Let's just store the offset directly (-30 to +30 in two's complement).
    .byte $00, $03, $06, $09, $0C, $0E, $11, $14, $16, $18, $1A, $1B, $1C, $1D, $1D, $1D
    .byte $1D, $1D, $1D, $1C, $1B, $1A, $18, $16, $14, $11, $0E, $0C, $09, $06, $03, $00
    .byte $00, $FD, $FA, $F7, $F4, $F2, $EF, $EC, $EA, $E8, $E6, $E5, $E4, $E3, $E3, $E3
    .byte $E3, $E3, $E3, $E4, $E5, $E6, $E8, $EA, $EC, $EF, $F2, $F4, $F7, $FA, $FD, $00

.segment "CHARS"
    ; Tile 0: Empty
    .res 16, $00
    ; Tile 1: Basic Ship Triangle
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00001000
    .byte %00011100
    .byte %00111110
    .byte %01111111

    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00001000
    .byte %00011100
    .byte %00111110
    .byte %01111111
    
    ; Tile 2: Bullet
    .byte %00010000
    .byte %00111000
    .byte %00111000
    .byte %00111000
    .byte %00111000
    .byte %00111000
    .byte %00111000
    .byte %00010000
    .res 8, $00
    
    ; Tile 3: Obsolete Enemy
    .res 16, $00
    
    ; Tile 4: Player Top-Left
    .byte %00000000
    .byte %00000011
    .byte %00000111
    .byte %00001111
    .byte %00011111
    .byte %00111111
    .byte %01111111
    .byte %11011111

    .byte %00000000
    .byte %00000011
    .byte %00000111
    .byte %00000111
    .byte %00000011
    .byte %00000010
    .byte %00000000
    .byte %00100000
    
    ; Tile 5: Player Bottom-Left
    .byte %10001111
    .byte %01001111
    .byte %00111100
    .byte %00111000
    .byte %00010000
    .byte %00001000
    .byte %00001000
    .byte %00000000
    
    .byte %01110000
    .byte %00110000
    .byte %00000000
    .byte %00000000
    .byte %00001000
    .byte %00001000
    .byte %00001000
    .byte %00000000

    ; Tile 6: Enemy Straight Top-Left (Heavy Armor)
    .byte %00000000
    .byte %00111111
    .byte %01110011
    .byte %11000001
    .byte %10000011
    .byte %10000011
    .byte %10000111
    .byte %10000111

    .byte %00000000
    .byte %00111111
    .byte %00111111
    .byte %01111111
    .byte %00111111
    .byte %00111111
    .byte %00111111
    .byte %00111111
    
    ; Tile 7: Enemy Straight Bottom-Left
    .byte %10000111
    .byte %10001111
    .byte %11011111
    .byte %01111111
    .byte %00111000
    .byte %00011000
    .byte %00001000
    .byte %00000000

    .byte %00111111
    .byte %00111111
    .byte %01111111
    .byte %00111111
    .byte %00111000
    .byte %00011000
    .byte %00001000
    .byte %00000000

    ; Tile 8: Enemy Diagonal Top-Left (Sleek)
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000111
    .byte %00001111
    .byte %00011110
    .byte %00111100
    .byte %01111000

    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000111
    .byte %00001111
    .byte %00011111
    .byte %00111111
    .byte %01111111
    
    ; Tile 9: Enemy Diagonal Bottom-Left
    .byte %11110000
    .byte %11100000
    .byte %01100000
    .byte %00100000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000000

    .byte %11111000
    .byte %11110000
    .byte %01100000
    .byte %00100000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    
    ; Tile 10: Enemy Sine Top-Left (Round UFO)
    .byte %00000000
    .byte %00000111
    .byte %00011111
    .byte %00110000
    .byte %01100111
    .byte %11001111
    .byte %10011111
    .byte %10111000

    .byte %00000000
    .byte %00000111
    .byte %00011111
    .byte %00111111
    .byte %01101111
    .byte %11001111
    .byte %10011111
    .byte %10111111
    
    ; Tile 11: Enemy Sine Bottom-Left
    .byte %10111000
    .byte %10011111
    .byte %01001110
    .byte %00110000
    .byte %00011100
    .byte %00000000
    .byte %00000000
    .byte %00000000

    .byte %10111111
    .byte %10011111
    .byte %01001111
    .byte %00111111
    .byte %00011100
    .byte %00000000
    .byte %00000000
    .byte %00000000

    ; Padding up to $10
    .res 64, $00

    ; Tile 16 ($10): '0'
    .byte %00011100
    .byte %00100010
    .byte %01000001
    .byte %01000001
    .byte %01000001
    .byte %00100010
    .byte %00011100
    .byte %00000000
    .res 8, $00
    ; Tile 17 ($11): '1'
    .byte %00001000
    .byte %00011000
    .byte %00001000
    .byte %00001000
    .byte %00001000
    .byte %00001000
    .byte %00011100
    .byte %00000000
    .res 8, $00
    ; Tile 18 ($12): '2'
    .byte %00011100
    .byte %00100010
    .byte %00000100
    .byte %00001000
    .byte %00010000
    .byte %00100000
    .byte %00111110
    .byte %00000000
    .res 8, $00
    ; Tile 19 ($13): '3'
    .byte %00011100
    .byte %00100010
    .byte %00000100
    .byte %00011000
    .byte %00000100
    .byte %00100010
    .byte %00011100
    .byte %00000000
    .res 8, $00
    ; Tile 20 ($14): '4'
    .byte %00000100
    .byte %00001100
    .byte %00010100
    .byte %00100100
    .byte %00111110
    .byte %00000100
    .byte %00000100
    .byte %00000000
    .res 8, $00
    ; Tile 21 ($15): '5'
    .byte %00111110
    .byte %00100000
    .byte %00111100
    .byte %00000010
    .byte %00000010
    .byte %00100010
    .byte %00011100
    .byte %00000000
    .res 8, $00
    ; Tile 22 ($16): '6'
    .byte %00001100
    .byte %00010000
    .byte %00100000
    .byte %00111100
    .byte %00100010
    .byte %00100010
    .byte %00011100
    .byte %00000000
    .res 8, $00
    ; Tile 23 ($17): '7'
    .byte %00111110
    .byte %00000010
    .byte %00000100
    .byte %00001000
    .byte %00010000
    .byte %00010000
    .byte %00010000
    .byte %00000000
    .res 8, $00
    ; Tile 24 ($18): '8'
    .byte %00011100
    .byte %00100010
    .byte %00100010
    .byte %00011100
    .byte %00100010
    .byte %00100010
    .byte %00011100
    .byte %00000000
    .res 8, $00
    ; Tile 25 ($19): '9'
    .byte %00011100
    .byte %00100010
    .byte %00100010
    .byte %00011110
    .byte %00000010
    .byte %00000100
    .byte %00011000
    .byte %00000000
    .res 8, $00

    ; Padding the rest of CHR
    .res 8192-416, $00
