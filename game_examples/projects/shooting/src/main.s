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

; For bullets
bullet_active: .res 4
bullet_x:      .res 4
bullet_y:      .res 4

; For enemy
enemy_active:  .res 1
enemy_x:       .res 1
enemy_y:       .res 1

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

    ; Sprite Palette 1 (3F14) - Enemy
    lda #$3F
    sta $2006
    lda #$14
    sta $2006
    lda #$0F       ; Transparent
    sta $2007
    lda #$21       ; Blue
    sta $2007
    lda #$11       ; Light blue
    sta $2007
    lda #$30       ; White
    sta $2007

    ; Initial state
    lda #$80
    sta player_x
    lda #$C0
    sta player_y
    lda #$00
    sta nmi_ready

    ldx #$00
@clear_bullets:
    sta bullet_active, x
    inx
    cpx #$04
    bcc @clear_bullets

    ; Init first enemy
    lda #$01
    sta enemy_active
    lda #$80
    sta enemy_x
    lda #$10
    sta enemy_y

    ; Enable NMI and rendering
    lda #$80      ; changed from $88 so sprite table is $0000
    sta $2000
    lda #$1E
    sta $2001

main_loop:
    inc nmi_ready
@wait:
    lda nmi_ready
    bne @wait

    jsr read_controller
    jsr update_player
    jsr update_bullets
    jsr update_enemy
    jsr check_collisions
    jsr draw_player
    jsr draw_enemy

    jmp main_loop
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
    ; ... (existing movement) ...
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
    ; Only fire if previous was not fired (need a simple cooldown/debounce, but let's just do a basic one)
    ; For simple debounce we could check but let's just find empty slot
    ldx #$00
@find_bullet:
    lda bullet_active, x
    bne @next_bullet
    ; Found empty slot!
    lda #$01
    sta bullet_active, x
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
    
    ; Off screen?
    cmp #$08
    bcs @skip
    lda #$00
    sta bullet_active, x

@skip:
    inx
    cpx #$04
    bcc @loop
    rts
.endproc

.proc draw_player
    ; Sprite 0 - Top Left
    lda player_y
    sta OAM_RAM+0
    lda #$01      ; Tile 1
    sta OAM_RAM+1
    lda #$00      ; Palette 0
    sta OAM_RAM+2
    lda player_x
    sta OAM_RAM+3

    ; Sprite 1 - Top Right
    lda player_y
    sta OAM_RAM+4
    lda #$01
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
    lda #$01
    sta OAM_RAM+9
    lda #$80      ; Flip V
    sta OAM_RAM+10
    lda player_x
    sta OAM_RAM+11

    ; Sprite 3 - Bottom Right
    lda player_y
    clc
    adc #$08
    sta OAM_RAM+12
    lda #$01
    sta OAM_RAM+13
    lda #$C0      ; Flip H+V
    sta OAM_RAM+14
    lda player_x
    clc
    adc #$08
    sta OAM_RAM+15

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

.proc update_enemy
    lda enemy_active
    beq @respawn
    
    ; Move enemy down
    lda enemy_y
    clc
    adc #$01
    sta enemy_y
    
    cmp #$EF
    bcc @done
@respawn:
    ; Respawn enemy
    lda #$01
    sta enemy_active
    lda #$10
    sta enemy_y
    ; simple "random" x
    lda nmi_ready
    and #$7F
    clc
    adc #$20
    sta enemy_x
@done:
    rts
.endproc

.proc check_collisions
    lda enemy_active
    beq @done
    
    ldx #$00
@loop:
    lda bullet_active, x
    beq @next
    
    ; check y collision
    lda bullet_y, x
    sec
    sbc enemy_y
    clc
    adc #$08       ; hit box buffer
    cmp #$10       ; 16 pixel height
    bcs @next
    
    ; check x collision
    lda bullet_x, x
    sec
    sbc enemy_x
    clc
    adc #$08
    cmp #$10
    bcs @next
    
    ; collision!
    lda #$00
    sta enemy_active
    sta bullet_active, x
    
@next:
    inx
    cpx #$04
    bcc @loop
@done:
    rts
.endproc

.proc draw_enemy
    lda enemy_active
    beq @hide
    
    lda enemy_y
    sta OAM_RAM+32
    lda #$03      ; Tile 3 for enemy
    sta OAM_RAM+33
    lda #$01      ; Palette 1
    sta OAM_RAM+34
    lda enemy_x
    sta OAM_RAM+35
    
    lda enemy_y
    sta OAM_RAM+36
    lda #$03
    sta OAM_RAM+37
    lda #$40      ; Flip H
    sta OAM_RAM+38
    lda enemy_x
    clc
    adc #$08
    sta OAM_RAM+39
    
    lda enemy_y
    clc
    adc #$08
    sta OAM_RAM+40
    lda #$03
    sta OAM_RAM+41
    lda #$80      ; Flip V
    sta OAM_RAM+42
    lda enemy_x
    sta OAM_RAM+43
    
    lda enemy_y
    clc
    adc #$08
    sta OAM_RAM+44
    lda #$03
    sta OAM_RAM+45
    lda #$C0      ; Flip H+V
    sta OAM_RAM+46
    lda enemy_x
    clc
    adc #$08
    sta OAM_RAM+47
    
    rts
    
@hide:
    lda #$FF
    sta OAM_RAM+32
    sta OAM_RAM+36
    sta OAM_RAM+40
    sta OAM_RAM+44
    rts
.endproc

.segment "VECTORS"
    .word nmi, reset, 0

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
    
    ; Tile 3: Enemy
    .byte %01111111
    .byte %00111110
    .byte %00011100
    .byte %00001000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000000

    .byte %01111111
    .byte %00111110
    .byte %00011100
    .byte %00001000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    .byte %00000000
    
    ; Padding the rest of CHR
    .res 8192-64, $00
