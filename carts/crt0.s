.section .v68hdr,"a"
.ascii	"V68\0"
.long	0
.long	_start
.long	0

.text
.global	_start

_start:
	lea	__data_load, %a0
	lea	__data_start, %a1
	move.l	#__data_end, %d0

0:	cmp.l	%d0, %a1
	beq	1f
	move.l	(%a0)+, (%a1)+
	bra	0b

1:	lea	__bss_start, %a0
	move.l	#__bss_end, %d0

2:	cmp.l	%d0, %a0
	beq	3f
	clr.l	(%a0)+
	bra	2b

3:	jsr	main
	rts
