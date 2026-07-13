#pragma once
#include <iostream>
#include <string>
#include"Channel.hpp"
#include "Actor.hpp"
#include "options.h"

extern void source_init();
extern unsigned char source_readByte();
extern int source_sizeOfFile();

class Source : public Actor {
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
	unsigned short octet_count;
	unsigned short octet_index;

	// Actor Parameters
	std::string actor_name;

	// Output Channels
	Data_Channel<unsigned char> *pl_bits; 

	// FSM
	enum class FSM {
		s_length,
		s_payload,
	};
	FSM state = FSM::s_length;

	void length(void) { 
		pl_bits->write(octet_count);
	}
	void payload(void) { 
		octet_index = octet_index+1;
		pl_bits->write(source_readByte());
	}
	void done(void) { 
		octet_index = 0;
		octet_count = source_sizeOfFile();
	}
public:
	Source(std::string _n, Data_Channel<unsigned char>* _pl_bits) {
		actor_name = _n;
		pl_bits = _pl_bits;
	};
	void schedule(void) { 
#ifdef PRINT_FIRINGS
		unsigned firings = 0;
#endif
		for(;;) {
			if (state == FSM::s_length) {
				if (true) {
					if ((octet_count>4)) {
						if ((pl_bits->free() >= 1)) {
							length(); 
#ifdef PRINT_FIRINGS
							++firings;
#endif
							state = FSM::s_payload;
						}
						else {
							break;
						}
					}
					else {
						break;
					}
				}
				else {
					break;
				}
			}
			else if (state == FSM::s_payload) {
				if (true) {
					if ((octet_index<octet_count)) {
						if ((pl_bits->free() >= 1)) {
							payload(); 
#ifdef PRINT_FIRINGS
							++firings;
#endif
							state = FSM::s_payload;
						}
						else {
							break;
						}
					}
					else if ((octet_index==octet_count)) {
						done(); 
#ifdef PRINT_FIRINGS
						++firings;
#endif
						state = FSM::s_length;
					}
					else {
						break;
					}
				}
				else {
					break;
				}
			}
		}
#ifdef PRINT_FIRINGS
		std::cout << actor_name << " fired " << firings << " times" << std::endl;
#endif
	}
	void initialize(void) {
		source_init();
		octet_index = 0;
		octet_count = source_sizeOfFile();
	}
};