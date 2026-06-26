#include "cpp/shim.hpp"

#include <string>

#include "Lexer/Lexer.hpp"
#include "Parser/Parser.hpp"

namespace shim
{

    Ast::~Ast() { delete root; }

    std::unique_ptr<Ast> parse_cal(rust::Str code)
    {
        std::string source(code.data(), code.size());
        Lexer::Lexer lexer{source};
        Parser::Parser_Class parser{&lexer};
        AST::AST_Root *root = parser.parse();
        return std::make_unique<Ast>(root);
    }

}
