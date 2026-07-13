#pragma once
#include <iostream>
#include <string>
#include"Channel.hpp"
#include "Actor.hpp"
#include "options.h"

extern void throw_away(int value);
extern void print_cyclecount();

class Sink : public Actor {
private:
	const int PL_DATA_SZ = 8;
	const int PAYLOAD_LEN_SZ = 9;
	const int BODY_SZ = 14;
	const int OFFSET_SZ = 5;
	const int SHIFT_SZ = 7;
	const int HSPCNT_SZ = 5;
	const int DATA_SZ = 8;
	const int CHIP_SZ = 32;
	const int SYMB_SZ = 8;
	const int HSP_SZ = 8;

	// Actor Parameters
	std::string actor_name;

	// Input Channels
	Data_Channel<bool> *done; 
	Data_Channel<char> *hsp; 

	void consume(void) { 
		char sample = hsp->read(); 
		throw_away(sample);
	}
	void finish(void) { 
		bool flag = done->read(); 
		print_cyclecount();
	}
public:
	Sink(std::string _n, Data_Channel<bool>* _done, Data_Channel<char>* _hsp) {
		actor_name = _n;
		done = _done;
		hsp = _hsp;
	};
	void schedule(void) { 
#ifdef PRINT_FIRINGS
		unsigned firings = 0;
#endif
		for(;;) {
			if ((hsp->size() >= 1)) {
				consume(); 
#ifdef PRINT_FIRINGS
				++firings;
#endif
			}
			else if ((done->size() >= 1)) {
				finish(); 
#ifdef PRINT_FIRINGS
				++firings;
#endif
			}
			else {
				break;
			}
		}
#ifdef PRINT_FIRINGS
		std::cout << actor_name << " fired " << firings << " times" << std::endl;
#endif
	}
	void initialize(void) {}
};