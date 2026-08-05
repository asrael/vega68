.section .vectors,"a"
.long	__stack_top
.long	_start
.rept	23	| 2..24: faults, reserved and spurious
.long	v68_fault
.endr
.rept	7	| 25..31: autovectors, including the app-facing line and vblank
.long	v68_rte_stub
.endr

.text
.global	_start

_start:
	lea	__data_load, %a0
	lea	__data_start, %a1
	lea	__data_end, %a2

0:	cmp.l	%a2, %a1
	beq	1f
	move.l	(%a0)+, (%a1)+
	bra	0b

1:	lea	__bss_start, %a0
	lea	__bss_end, %a1

2:	cmp.l	%a1, %a0
	beq	3f
	clr.l	(%a0)+
	bra	2b

3:	jsr	main

halt:
	bra	halt

.global	v68_fault

v68_fault:
	move.w	#0x2700, %sr
	movem.l	%d0-%d7/%a0-%a7, v68_fault_regs
	move.l	%sp, v68_fault_regs+64
	move.l	v68_monitor_sp, %sp
	jsr	v68_fault_dump

0:	bra	0b

.global	v68_rte_stub

v68_rte_stub:
	rte
